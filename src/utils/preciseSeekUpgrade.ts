/**
 * Coordinates FLAC-to-WAV upgrades for precise seeking through injected I/O:
 *
 * - Only one upgrade runs; repeated seeks collapse to the latest target.
 * - A seek generation discards requests that became stale before execution.
 * - Changed audio, track, or generation invalidates the result.
 * - Playback resumes only when no pause fade is still pending.
 */
export interface PreciseSeekSnapshot {
  howler: unknown;
  trackId: number;
  playing: boolean;
  pausePending: boolean;
  seekToken: number;
}

interface CachedFlac<TBytes> {
  bytes: TBytes;
  bitsPerSample?: number;
}

interface PreciseSeekDependencies<TBytes> {
  getSnapshot: () => PreciseSeekSnapshot;
  readCachedFlac: (trackId: number) => Promise<CachedFlac<TBytes> | null>;
  convertViaSidecar: (
    trackId: number,
    bytes: TBytes,
    bitsPerSample: number | undefined
  ) => Promise<string | null>;
  convertInRenderer: (bytes: TBytes) => Promise<string | null>;
  freezeAt: (time: number) => void;
  seekStream: (time: number) => void;
  applyPreciseSource: (url: string, time: number, resume: boolean) => void;
  onError?: (error: unknown) => void;
}

interface PreciseSeekRequest {
  time: number;
  token: number;
}

export interface PreciseSeekUpgrader {
  request(time: number): void;
  readonly busy: boolean;
}

export function createPreciseSeekUpgrader<TBytes>({
  getSnapshot,
  readCachedFlac,
  convertViaSidecar,
  convertInRenderer,
  freezeAt,
  seekStream,
  applyPreciseSource,
  onError = () => {},
}: PreciseSeekDependencies<TBytes>): PreciseSeekUpgrader {
  let pending: PreciseSeekRequest | null = null;
  let draining = false;

  function request(time: number): void {
    pending = {
      time: Math.max(0, Number(time) || 0),
      token: getSnapshot().seekToken,
    };
    if (!draining) void drain();
  }

  async function drain(): Promise<void> {
    draining = true;
    try {
      while (pending) {
        const req = pending;
        pending = null;
        // Any newer seek makes this queued target stale.
        if (getSnapshot().seekToken !== req.token) continue;
        await runOnce(req);
      }
    } finally {
      draining = false;
    }
  }

  const changedSince = (start: PreciseSeekSnapshot): boolean => {
    const now = getSnapshot();
    return (
      now.howler !== start.howler ||
      now.trackId !== start.trackId ||
      now.seekToken !== start.seekToken
    );
  };

  async function runOnce(req: PreciseSeekRequest): Promise<void> {
    const start = getSnapshot();
    // Freeze lyric timing while showing the requested position immediately.
    freezeAt(req.time);
    let url: string | null = null;
    try {
      const cached = await readCachedFlac(start.trackId);
      if (cached && !changedSince(start)) {
        url = await convertViaSidecar(
          start.trackId,
          cached.bytes,
          cached.bitsPerSample
        );
        if (!url && !changedSince(start)) {
          url = await convertInRenderer(cached.bytes);
        }
      }
    } catch (error) {
      onError(error);
      url = null;
    }

    const now = getSnapshot();
    if (now.howler !== start.howler || now.trackId !== start.trackId) {
      // Audio replacement owns cleanup, so this result is stale.
      return;
    }
    if (now.seekToken !== start.seekToken) {
      // A newer streaming seek has applied or queued another upgrade.
      return;
    }
    if (pending) return; // Let the loop retry with the latest target.
    if (!url) {
      // Fall back to streaming seek when cached conversion is unavailable.
      seekStream(req.time);
      return;
    }
    applyPreciseSource(url, req.time, now.playing && !now.pausePending);
  }

  return {
    request,
    get busy() {
      return draining;
    },
  };
}
