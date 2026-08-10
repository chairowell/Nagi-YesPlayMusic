import { describe, expect, test } from 'bun:test';
import {
  createNextTrackPrefetcher,
  warmTrackArtwork,
} from '../src/utils/trackPrefetch';
import {
  ARTWORK_SIZE,
  PREFETCHED_ARTWORK_SIZES,
  UPCOMING_ARTWORK_COUNT,
} from '../src/utils/artwork';
import { getUpcomingTrackIDs } from '../src/utils/playerQueue';

interface TestTrack {
  id: number;
  al: { picUrl: string };
}

interface TestPrefetchImage {
  decoding: 'async' | 'auto' | 'sync';
  fetchPriority: 'high' | 'low' | 'auto';
  src: string;
}

describe('下一首轻量预取', () => {
  test('同一目标去重，队列变化后淘汰旧响应', async () => {
    const resolvers = new Map<number, (track: TestTrack) => void>();
    const loadedLyrics: number[] = [];
    const warmedTracks: number[] = [];
    const prefetcher = createNextTrackPrefetcher({
      loadTrack: id =>
        new Promise<TestTrack>(resolve => {
          resolvers.set(id, resolve);
        }),
      loadLyric: id => loadedLyrics.push(id),
      warmArtwork: track => warmedTracks.push(track.id),
      cacheAudio: () => {
        throw new Error('未开启音频缓存时不应调用');
      },
    });

    const first = prefetcher.prefetch(101);
    await Promise.resolve();
    expect(prefetcher.prefetch(101)).toBe(first);
    const second = prefetcher.prefetch(202);
    await Promise.resolve();

    resolvers.get(101)?.({
      id: 101,
      al: { picUrl: 'http://example/1.jpg' },
    });
    resolvers.get(202)?.({
      id: 202,
      al: { picUrl: 'http://example/2.jpg' },
    });
    await Promise.all([first, second]);

    expect(loadedLyrics).toEqual([202]);
    expect(warmedTracks).toEqual([202]);
  });

  test('只为仍然有效的目标执行可选音频缓存', async () => {
    const cachedAudio: number[] = [];
    const prefetcher = createNextTrackPrefetcher({
      loadTrack: async id => ({ id, al: { picUrl: '' } }),
      loadLyric: async () => {},
      warmArtwork: () => {},
      cacheAudio: async track => cachedAudio.push(track.id),
    });

    await prefetcher.prefetch(303, { includeAudio: true });

    expect(cachedAudio).toEqual([303]);
  });

  test('封面预热使用 HTTPS，并只保留实际界面需要的尺寸', () => {
    const images: TestPrefetchImage[] = [];
    const urls = warmTrackArtwork(
      { al: { picUrl: 'http://p1.music.126.net/cover.jpg' } },
      () => {
        const image: TestPrefetchImage = {
          decoding: 'auto',
          fetchPriority: 'auto',
          src: '',
        };
        images.push(image);
        return image;
      }
    );

    expect(urls).toEqual([
      'https://p1.music.126.net/cover.jpg?param=128y128',
      'https://p1.music.126.net/cover.jpg?param=224y224',
      'https://p1.music.126.net/cover.jpg?param=512y512',
    ]);
    expect(images.map(image => image.src)).toEqual(urls);
    // Prefetch must not compete with lossless audio for bandwidth.
    expect(images.map(image => image.fetchPriority)).toEqual(
      urls.map(() => 'low')
    );
  });

  test('预取的尺寸覆盖迷你播放条实际要的那一档', () => {
    // Prefetch the exact sizes used as cache keys by each surface.
    expect(PREFETCHED_ARTWORK_SIZES).toContain(ARTWORK_SIZE.miniPlayer);
  });

  test('封面预热提前备好后面几首，而不是只备下一首', () => {
    // Prefetch two tracks so the next cover is ready before a quick skip.
    expect(UPCOMING_ARTWORK_COUNT).toBeGreaterThan(1);
    expect(getUpcomingTrackIDs([1, 2, 3, 4, 5], 0, 1, false, 3)).toEqual([
      2, 3, 4,
    ]);
    // Stop at the queue boundary instead of padding with undefined.
    expect(getUpcomingTrackIDs([1, 2, 3], 1, 1, false, 3)).toEqual([3]);
    // Stop when wrapping reaches the current track.
    expect(getUpcomingTrackIDs([1, 2], 1, 1, true, 3)).toEqual([1]);
    expect(getUpcomingTrackIDs([7], 0, 1, true, 3)).toEqual([]);
    // Prefetch backward when reverse playback is enabled.
    expect(getUpcomingTrackIDs([1, 2, 3, 4], 3, -1, false, 2)).toEqual([3, 2]);
  });
});
