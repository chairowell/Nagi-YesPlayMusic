import type { Track } from '@/types/domain';

export interface PlayerInfo {
  currentTrack: Track | null;
  progress: number;
}

function isUnknownRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isTrack(value: unknown): value is Track {
  return (
    isUnknownRecord(value) &&
    typeof value['id'] === 'number' &&
    Number.isFinite(value['id'])
  );
}

export function decodePlayerInfo(value: unknown): PlayerInfo | null {
  if (!isUnknownRecord(value)) return null;
  const currentTrack = value['currentTrack'];
  const progress = value['progress'];
  if (currentTrack !== null && !isTrack(currentTrack)) return null;
  if (
    typeof progress !== 'number' ||
    !Number.isFinite(progress) ||
    progress < 0
  ) {
    return null;
  }
  return { currentTrack, progress };
}

export function initialPlayerInfo(): PlayerInfo {
  return { currentTrack: null, progress: 0 };
}
