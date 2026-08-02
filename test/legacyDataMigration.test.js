import { describe, expect, test } from 'bun:test';
import { migrateLegacyDesktopSettings } from '../src/services/legacyDataMigration';

function createStorage(initial = {}) {
  const values = new Map(Object.entries(initial));
  return {
    getItem: key => values.get(key) ?? null,
    setItem: (key, value) => values.set(key, String(value)),
    values,
  };
}

describe('Electron → Tauri 设置迁移', () => {
  test('首次启动只导入小体积设置并保留新版默认字段', async () => {
    const storage = createStorage();
    const migrated = await migrateLegacyDesktopSettings({
      isTauri: true,
      storage,
      loadLegacySettings: async () => ({
        lang: 'zh-CN',
        cacheLimit: null,
      }),
    });

    expect(migrated).toBe(true);
    expect(JSON.parse(storage.getItem('settings'))).toMatchObject({
      lang: 'zh-CN',
      cacheLimit: null,
      enableGlobalShortcut: true,
    });
    expect(JSON.parse(storage.getItem('data'))).toEqual({
      user: {},
      likedSongPlaylistID: 0,
      lastRefreshCookieDate: 0,
      loginMode: null,
    });
    expect(storage.getItem('legacyElectronSettingsImportedV1')).toBe('done');
  });

  test('已有 Tauri 数据时绝不拿旧版设置覆盖', async () => {
    const storage = createStorage({
      appVersion: '0.5.0',
      settings: JSON.stringify({ lang: 'tr' }),
    });
    let readCount = 0;
    const migrated = await migrateLegacyDesktopSettings({
      isTauri: true,
      storage,
      loadLegacySettings: async () => {
        readCount += 1;
        return { lang: 'zh-CN' };
      },
    });

    expect(migrated).toBe(false);
    expect(readCount).toBe(0);
    expect(JSON.parse(storage.getItem('settings'))).toEqual({ lang: 'tr' });
    expect(storage.getItem('legacyElectronSettingsImportedV1')).toBe('skipped');
  });

  test('Electron 和普通 Web 不读取本机旧版目录', async () => {
    const storage = createStorage();
    let readCount = 0;
    const migrated = await migrateLegacyDesktopSettings({
      isTauri: false,
      storage,
      loadLegacySettings: async () => {
        readCount += 1;
        return {};
      },
    });

    expect(migrated).toBe(false);
    expect(readCount).toBe(0);
  });
});
