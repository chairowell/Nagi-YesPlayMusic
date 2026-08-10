export interface TrackSwitchGuard {
  begin(): number;
  isCurrent(candidate: number): boolean;
}

interface TrackSwitchOperations<TTrack, TSource> {
  onBegin?: () => void;
  loadTrack: () => Promise<TTrack>;
  commitTrack: (track: TTrack) => void;
  loadSource: (track: TTrack) => Promise<TSource | null>;
  commitSource: (source: TSource, track: TTrack) => void;
  onMissingSource?: (track: TTrack) => void;
}

export function createTrackSwitchGuard(): Readonly<TrackSwitchGuard> {
  let generation = 0;

  return Object.freeze({
    begin() {
      generation += 1;
      return generation;
    },
    isCurrent(candidate: number) {
      return candidate === generation;
    },
  });
}

/**
 * Track details and audio sources share one generation guard. A track ID is
 * unavailable until details resolve, so it cannot identify every stale request.
 */
export async function runLatestTrackSwitch<TTrack, TSource>(
  guard: TrackSwitchGuard,
  {
    onBegin,
    loadTrack,
    commitTrack,
    loadSource,
    commitSource,
    onMissingSource,
  }: TrackSwitchOperations<TTrack, TSource>
): Promise<boolean> {
  const generation = guard.begin();
  onBegin?.();

  const track = await loadTrack();
  if (!guard.isCurrent(generation)) return false;

  commitTrack(track);
  const source = await loadSource(track);
  if (!guard.isCurrent(generation)) return false;

  if (!source) {
    onMissingSource?.(track);
    return false;
  }

  commitSource(source, track);
  return true;
}
