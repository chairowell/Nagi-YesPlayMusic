import { PREFETCHED_ARTWORK_SIZES, buildArtworkURL } from '@/utils/artwork';

interface ArtworkTrack {
  id?: number;
  al?: { picUrl?: string };
  album?: { picUrl?: string };
}

interface PrefetchImage {
  decoding: 'async' | 'auto' | 'sync';
  fetchPriority: 'high' | 'low' | 'auto';
  src: string;
}

interface NextTrackPrefetchDependencies<TTrack extends ArtworkTrack> {
  loadTrack: (trackID: number) => TTrack | null | Promise<TTrack | null>;
  loadLyric: (trackID: number) => unknown;
  warmArtwork: (track: TTrack) => unknown;
  cacheAudio: (track: TTrack, isCurrent: () => boolean) => unknown;
}

interface PrefetchOptions {
  includeAudio?: boolean;
}

function artworkURLs(track: ArtworkTrack): string[] {
  const rawURL = track?.al?.picUrl ?? track?.album?.picUrl;
  return PREFETCHED_ARTWORK_SIZES.map(size =>
    buildArtworkURL(rawURL, size)
  ).filter(Boolean);
}

/**
 * Warm WebKit's image cache without retaining Image objects or decoding them.
 * Low priority prevents prefetches from competing with lossless audio.
 */
export function warmTrackArtwork(
  track: ArtworkTrack,
  createImage: () => PrefetchImage = () => new Image()
): string[] {
  const urls = artworkURLs(track);
  urls.forEach(src => {
    const image = createImage();
    image.decoding = 'async';
    image.fetchPriority = 'low';
    image.src = src;
  });
  return urls;
}

/**
 * One generation guard prevents stale queue details from fetching more data.
 */
export function createNextTrackPrefetcher<TTrack extends ArtworkTrack>({
  loadTrack,
  loadLyric,
  warmArtwork,
  cacheAudio,
}: NextTrackPrefetchDependencies<TTrack>) {
  let generation = 0;
  let targetKey: string | null = null;
  let currentTask: Promise<TTrack | null> | null = null;

  const clear = (): void => {
    generation += 1;
    targetKey = null;
    currentTask = null;
  };

  const prefetch = (
    trackID: number | null | undefined,
    { includeAudio = false }: PrefetchOptions = {}
  ): Promise<TTrack | null> => {
    if (!trackID) {
      clear();
      return Promise.resolve(null);
    }

    const nextKey = `${trackID}:${includeAudio ? 'audio' : 'basic'}`;
    if (nextKey === targetKey && currentTask) return currentTask;

    targetKey = nextKey;
    const requestGeneration = ++generation;
    const isCurrent = () =>
      requestGeneration === generation && targetKey === nextKey;

    currentTask = Promise.resolve()
      .then(() => loadTrack(trackID))
      .then(async track => {
        if (!track || !isCurrent()) return null;

        const tasks: Promise<unknown>[] = [
          Promise.resolve().then(() => loadLyric(trackID)),
          Promise.resolve().then(() => warmArtwork(track)),
        ];
        if (includeAudio) {
          tasks.push(
            Promise.resolve().then(() => cacheAudio(track, isCurrent))
          );
        }
        await Promise.allSettled(tasks);
        return isCurrent() ? track : null;
      })
      .catch(error => {
        if (isCurrent()) {
          console.debug('[Player] 下一首预取失败，切歌时按正常路径加载', error);
        }
        return null;
      });

    return currentTask;
  };

  return { clear, prefetch };
}
