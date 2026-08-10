import { describe, expect, test } from 'bun:test';
import { decodePlayerInfo, initialPlayerInfo } from '@/services/playerInfo';

describe('local player status API', () => {
  test('accepts a finite player snapshot', () => {
    const track = { id: 42, name: 'Track' };
    expect(decodePlayerInfo({ currentTrack: track, progress: 12.5 })).toEqual({
      currentTrack: track,
      progress: 12.5,
    });
  });

  test('rejects malformed external payloads', () => {
    expect(decodePlayerInfo(null)).toBeNull();
    expect(
      decodePlayerInfo({ currentTrack: { id: '42' }, progress: 1 })
    ).toBeNull();
    expect(
      decodePlayerInfo({ currentTrack: { id: 42 }, progress: Number.NaN })
    ).toBeNull();
    expect(decodePlayerInfo({ currentTrack: null, progress: -1 })).toBeNull();
  });

  test('starts with an explicit empty snapshot', () => {
    expect(initialPlayerInfo()).toEqual({ currentTrack: null, progress: 0 });
  });
});
