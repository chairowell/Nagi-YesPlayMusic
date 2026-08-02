import initLocalStorage from '@/store/initLocalStorage';
import { isTauriRuntime } from '@/utils/runtime';
import pkg from '../../package.json';

const MIGRATION_MARKER = 'legacyElectronSettingsImportedV1';

async function readLegacySettingsFromTauri() {
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke('read_legacy_settings');
}

export async function migrateLegacyDesktopSettings({
  isTauri = isTauriRuntime,
  storage = localStorage,
  loadLegacySettings = readLegacySettingsFromTauri,
} = {}) {
  if (!isTauri || storage.getItem(MIGRATION_MARKER) !== null) return false;

  // 已经使用过 Tauri 的人可能主动改过设置；任何旧版快照都不应覆盖它。
  if (storage.getItem('appVersion') !== null) {
    storage.setItem(MIGRATION_MARKER, 'skipped');
    return false;
  }

  try {
    const legacySettings = await loadLegacySettings();
    if (!legacySettings || typeof legacySettings !== 'object') {
      storage.setItem(MIGRATION_MARKER, 'missing');
      return false;
    }

    storage.setItem(
      'settings',
      JSON.stringify({ ...initLocalStorage.settings, ...legacySettings })
    );
    // Electron 主进程没有完整播放队列和用户数据快照；写入安全默认值，
    // 后续登录后由 API 恢复资料，且绝不搬运巨大的 IndexedDB 音频缓存。
    storage.setItem('data', JSON.stringify(initLocalStorage.data));
    storage.setItem('player', JSON.stringify(initLocalStorage.player));
    storage.setItem('appVersion', JSON.stringify(pkg.version));
    storage.setItem(MIGRATION_MARKER, 'done');
    return true;
  } catch (error) {
    // 旧版仍在运行、文件暂时不可读时保留重试机会，不阻塞新应用启动。
    console.warn('[migration] Electron 设置暂时无法读取', error);
    return false;
  }
}
