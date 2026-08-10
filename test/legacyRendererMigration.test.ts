import { describe, expect, test } from 'bun:test';
import { migrateLegacyRendererData } from '../src/services/legacyRendererMigration';
import {
  createMemoryStorage,
  requireStoredItem,
} from './helpers/memoryStorage';

function nativeResult(overrides: Record<string, unknown> = {}) {
  return {
    localStorage: {},
    cookiesImported: 0,
    encryptedCookiesSkipped: 0,
    cookiesFailed: 0,
    authCookieSource: 'none',
    cacheDetected: false,
    ...overrides,
  };
}

describe('Electron renderer data migration', () => {
  test('imports validated player, account, Last.fm, progress, and cookies', async () => {
    const storage = createMemoryStorage();
    const result = await migrateLegacyRendererData({
      isTauri: true,
      storage,
      loadLegacyData: async () =>
        nativeResult({
          localStorage: {
            data: JSON.stringify({
              user: { userId: 42, nickname: 'legacy' },
              loginMode: 'account',
              lastRefreshCookieDate: 1,
            }),
            lastfm: JSON.stringify({ key: 'session', name: 'legacy' }),
            player: JSON.stringify({ _current: 2, _list: [1, 2, 3] }),
            playerCurrentTrackTime: '12.5',
          },
          cookiesImported: 3,
          authCookieSource: 'legacy',
        }),
    });

    expect(result).toEqual({
      status: 'completed',
      migratedKeys: ['data', 'lastfm', 'player', 'playerCurrentTrackTime'],
      failedKeys: [],
      cookiesImported: 3,
      notice: 'complete',
    });
    expect(JSON.parse(requireStoredItem(storage, 'data'))).toMatchObject({
      user: { userId: 42, nickname: 'legacy' },
      loginMode: 'account',
    });
    expect(JSON.parse(requireStoredItem(storage, 'lastfm'))).toEqual({
      key: 'session',
      name: 'legacy',
    });
    expect(JSON.parse(requireStoredItem(storage, 'player'))).toEqual({
      _current: 2,
      _list: [1, 2, 3],
    });
    expect(storage.getItem('playerCurrentTrackTime')).toBe('12.5');
  });

  test('reports credentials and cache that cannot be migrated', async () => {
    const storage = createMemoryStorage();
    const result = await migrateLegacyRendererData({
      isTauri: true,
      storage,
      loadLegacyData: async () =>
        nativeResult({
          localStorage: {
            data: JSON.stringify({
              user: { userId: 42, nickname: 'legacy' },
              loginMode: 'account',
            }),
          },
          encryptedCookiesSkipped: 1,
          cacheDetected: true,
        }),
    });

    expect(result?.status).toBe('completed');
    expect(result?.status === 'completed' ? result.notice : null).toBe(
      'login-and-cache'
    );
    expect(storage.getItem('legacyElectronRendererImportedV1')).toBe(
      'login-and-cache'
    );
  });

  test('does not treat unrelated cookies as a restored login', async () => {
    const storage = createMemoryStorage();
    const result = await migrateLegacyRendererData({
      isTauri: true,
      storage,
      loadLegacyData: async () =>
        nativeResult({
          localStorage: {
            data: JSON.stringify({
              user: { userId: 42, nickname: 'legacy' },
              loginMode: 'account',
            }),
          },
          cookiesImported: 3,
          authCookieSource: 'none',
        }),
    });

    expect(result?.status).toBe('completed');
    expect(result?.status === 'completed' ? result.notice : null).toBe(
      'login-required'
    );
  });

  test('username lookup mode does not require an account cookie', async () => {
    const storage = createMemoryStorage();
    const result = await migrateLegacyRendererData({
      isTauri: true,
      storage,
      loadLegacyData: async () =>
        nativeResult({
          localStorage: {
            data: JSON.stringify({
              user: { userId: 42, nickname: 'lookup-only' },
              loginMode: 'username',
            }),
          },
        }),
    });

    expect(result?.status).toBe('completed');
    expect(result?.status === 'completed' ? result.notice : null).toBe(
      'complete'
    );
  });

  test('an existing Tauri cookie never inherits the Electron account identity', async () => {
    const storage = createMemoryStorage();
    const result = await migrateLegacyRendererData({
      isTauri: true,
      storage,
      loadLegacyData: async () =>
        nativeResult({
          localStorage: {
            data: JSON.stringify({
              user: { userId: 42, nickname: 'electron-account' },
              loginMode: 'account',
              likedSongPlaylistID: 99,
            }),
          },
          authCookieSource: 'existing',
        }),
    });

    expect(result?.status).toBe('completed');
    expect(JSON.parse(requireStoredItem(storage, 'data'))).toMatchObject({
      user: {},
      loginMode: 'account',
      likedSongPlaylistID: 0,
    });
  });

  test('never overwrites an existing Tauri profile', async () => {
    const storage = createMemoryStorage({
      appVersion: '0.6.3',
      player: JSON.stringify({ _current: 9 }),
    });
    let readCount = 0;
    const result = await migrateLegacyRendererData({
      isTauri: true,
      storage,
      loadLegacyData: async () => {
        readCount += 1;
        return nativeResult();
      },
    });

    expect(result).toBeNull();
    expect(readCount).toBe(0);
    expect(JSON.parse(requireStoredItem(storage, 'player'))).toEqual({
      _current: 9,
    });
  });

  test('rejects malformed native payloads without spreading unknown data', async () => {
    const storage = createMemoryStorage();
    const result = await migrateLegacyRendererData({
      isTauri: true,
      storage,
      loadLegacyData: async () =>
        nativeResult({ localStorage: { player: ['not', 'a', 'string'] } }),
    });

    expect(result).toEqual({ status: 'retry-required' });
    expect(storage.getItem('player')).toBeNull();
    expect(storage.getItem('legacyElectronRendererImportedV1')).toBeNull();
  });

  test('reports each malformed legacy value as a partial import', async () => {
    for (const [key, value] of [
      ['data', '{'],
      ['lastfm', 'null'],
      ['player', '[]'],
      ['playerCurrentTrackTime', 'not-a-number'],
    ] as const) {
      const storage = createMemoryStorage();
      const result = await migrateLegacyRendererData({
        isTauri: true,
        storage,
        loadLegacyData: async () =>
          nativeResult({ localStorage: { [key]: value } }),
      });

      expect(result?.status).toBe('completed');
      expect(result?.status === 'completed' ? result.failedKeys : []).toEqual([
        key,
      ]);
      expect(result?.status === 'completed' ? result.notice : null).toBe(
        'partial-import'
      );
    }
  });

  test('keeps transient native failures retryable', async () => {
    const storage = createMemoryStorage();
    let attempts = 0;
    const options = {
      isTauri: true,
      storage,
      loadLegacyData: async () => {
        attempts += 1;
        if (attempts === 1) throw new Error('profile is locked');
        return nativeResult({
          localStorage: {
            data: JSON.stringify({
              user: { userId: 42, nickname: 'legacy' },
              loginMode: 'account',
            }),
            player: JSON.stringify({ _current: 2, _list: [1, 2, 3] }),
            playerCurrentTrackTime: '8.5',
          },
          authCookieSource: 'legacy',
        });
      },
    };

    const firstResult = await migrateLegacyRendererData(options);
    // Bootstrap stops here, so Tauri cannot seed values that mask legacy state.
    const secondResult = await migrateLegacyRendererData(options);

    expect(firstResult).toEqual({ status: 'retry-required' });
    expect(secondResult?.status).toBe('completed');
    expect(
      secondResult?.status === 'completed' ? secondResult.migratedKeys : []
    ).toEqual(['data', 'player', 'playerCurrentTrackTime']);
    expect(JSON.parse(requireStoredItem(storage, 'data'))).toMatchObject({
      user: { userId: 42, nickname: 'legacy' },
      loginMode: 'account',
    });
    expect(storage.getItem('playerCurrentTrackTime')).toBe('8.5');
    expect(attempts).toBe(2);
  });
});
