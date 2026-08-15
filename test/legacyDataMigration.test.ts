import { describe, expect, test } from 'bun:test';
import { migrateLegacyDesktopSettings } from '../src/services/legacyDataMigration';
import defaultStorageState from '../src/stores/defaults';
import {
  createMemoryStorage,
  requireStoredItem,
} from './helpers/memoryStorage';

describe('Electron → Tauri 设置迁移', () => {
  test('首次启动只标记 settings-only，不伪造 renderer 数据', async () => {
    const storage = createMemoryStorage();
    const migrated = await migrateLegacyDesktopSettings({
      isTauri: true,
      storage,
      loadLegacySettings: async () => ({
        lang: 'zh-CN',
        cacheLimit: null,
      }),
    });

    expect(migrated).toBe(true);
    expect(JSON.parse(requireStoredItem(storage, 'settings'))).toMatchObject({
      lang: 'zh-CN',
      cacheLimit: 8192,
      enableGlobalShortcut: true,
    });
    expect(storage.getItem('data')).toBeNull();
    expect(storage.getItem('player')).toBeNull();
    expect(storage.getItem('legacyElectronSettingsImportedV1')).toBe(
      'settings-only'
    );
  });

  test('已有 Tauri 数据时绝不拿旧版设置覆盖', async () => {
    const storage = createMemoryStorage({
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
    expect(JSON.parse(requireStoredItem(storage, 'settings'))).toEqual({
      lang: 'tr',
    });
    expect(storage.getItem('legacyElectronSettingsImportedV1')).toBe('skipped');
  });

  test('数组根节点不会被 spread 进设置', async () => {
    const storage = createMemoryStorage();
    const migrated = await migrateLegacyDesktopSettings({
      isTauri: true,
      storage,
      loadLegacySettings: async () => ['appearance', 'dark'],
    });

    expect(migrated).toBe(false);
    expect(storage.getItem('settings')).toBeNull();
    expect(storage.getItem('legacyElectronSettingsImportedV1')).toBe('invalid');
  });

  test('畸形字段回退到严格默认值', async () => {
    const storage = createMemoryStorage();
    const migrated = await migrateLegacyDesktopSettings({
      isTauri: true,
      storage,
      loadLegacySettings: async () => ({
        appearance: ['dark'],
        cacheLimit: 'unlimited',
        closeAppOption: 'force',
        shortcuts: 'Command+P',
      }),
    });

    expect(migrated).toBe(true);
    expect(JSON.parse(requireStoredItem(storage, 'settings'))).toMatchObject({
      appearance: defaultStorageState.settings.appearance,
      cacheLimit: defaultStorageState.settings.cacheLimit,
      closeAppOption: defaultStorageState.settings.closeAppOption,
      shortcuts: defaultStorageState.settings.shortcuts,
    });
  });

  test('普通 Web 不读取本机旧版目录', async () => {
    const storage = createMemoryStorage();
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
