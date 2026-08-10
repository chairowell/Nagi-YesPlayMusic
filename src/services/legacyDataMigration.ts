import defaultStorageState from '@/stores/defaults';
import { isTauriRuntime } from '@/utils/runtime';
import { decodeSettingsState, isUnknownRecord } from '@/utils/persistedState';
import pkg from '../../package.json';

const MIGRATION_MARKER = 'legacyElectronSettingsImportedV1';

interface LegacyMigrationOptions {
  isTauri?: boolean;
  storage?: Pick<Storage, 'getItem' | 'setItem'>;
  loadLegacySettings?: () => Promise<unknown>;
}

async function readLegacySettingsFromTauri(): Promise<unknown> {
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke<unknown>('read_legacy_settings');
}

export async function migrateLegacyDesktopSettings({
  isTauri = isTauriRuntime,
  storage = localStorage,
  loadLegacySettings = readLegacySettingsFromTauri,
}: LegacyMigrationOptions = {}): Promise<boolean> {
  if (!isTauri || storage.getItem(MIGRATION_MARKER) !== null) return false;

  // Never overwrite settings already created or changed in Tauri.
  if (storage.getItem('appVersion') !== null) {
    storage.setItem(MIGRATION_MARKER, 'skipped');
    return false;
  }

  try {
    const legacySettings = await loadLegacySettings();
    if (legacySettings === null || legacySettings === undefined) {
      storage.setItem(MIGRATION_MARKER, 'missing');
      return false;
    }
    if (!isUnknownRecord(legacySettings)) {
      storage.setItem(MIGRATION_MARKER, 'invalid');
      return false;
    }

    storage.setItem(
      'settings',
      JSON.stringify(
        decodeSettingsState(legacySettings, defaultStorageState.settings)
      )
    );
    // Renderer-origin data requires a separate migration path.
    storage.setItem('appVersion', JSON.stringify(pkg.version));
    storage.setItem(MIGRATION_MARKER, 'settings-only');
    return true;
  } catch (error) {
    // Keep migration retryable when the legacy app temporarily locks its data.
    console.warn('[migration] Electron 设置暂时无法读取', error);
    return false;
  }
}
