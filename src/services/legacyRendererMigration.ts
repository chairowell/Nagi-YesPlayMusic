import defaultStorageState from '@/stores/defaults';
import {
  decodeDataState,
  decodeLastfmState,
  decodeStoredRecord,
  isUnknownRecord,
} from '@/utils/persistedState';
import { isTauriRuntime } from '@/utils/runtime';

const MIGRATION_MARKER = 'legacyElectronRendererImportedV1';
const MIGRATED_STORAGE_KEYS = [
  'data',
  'lastfm',
  'player',
  'playerCurrentTrackTime',
] as const;

type MigratedStorageKey = (typeof MIGRATED_STORAGE_KEYS)[number];

export type LegacyMigrationNotice =
  | 'complete'
  | 'partial-import'
  | 'cache-not-migrated'
  | 'login-required'
  | 'login-and-cache';

export type LegacyRendererMigrationResult =
  | {
      status: 'completed';
      migratedKeys: string[];
      failedKeys: MigratedStorageKey[];
      cookiesImported: number;
      notice: LegacyMigrationNotice | null;
    }
  | { status: 'retry-required' };

interface LegacyRendererMigrationOptions {
  isTauri?: boolean;
  storage?: Pick<Storage, 'getItem' | 'setItem'>;
  loadLegacyData?: () => Promise<unknown>;
}

interface NativeLegacyRendererData {
  localStorage: Record<string, string>;
  cookiesImported: number;
  encryptedCookiesSkipped: number;
  cookiesFailed: number;
  authCookieSource: 'existing' | 'legacy' | 'none';
  cacheDetected: boolean;
}

function nonNegativeInteger(value: unknown): number | null {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0
    ? value
    : null;
}

function decodeNativeResult(value: unknown): NativeLegacyRendererData | null {
  if (!isUnknownRecord(value) || !isUnknownRecord(value['localStorage'])) {
    return null;
  }
  const localStorage: Record<string, string> = {};
  for (const [key, item] of Object.entries(value['localStorage'])) {
    if (typeof item !== 'string') return null;
    localStorage[key] = item;
  }
  const cookiesImported = nonNegativeInteger(value['cookiesImported']);
  const encryptedCookiesSkipped = nonNegativeInteger(
    value['encryptedCookiesSkipped']
  );
  const cookiesFailed = nonNegativeInteger(value['cookiesFailed']);
  const authCookieSource = value['authCookieSource'];
  if (
    cookiesImported === null ||
    encryptedCookiesSkipped === null ||
    cookiesFailed === null ||
    typeof value['cacheDetected'] !== 'boolean' ||
    (authCookieSource !== 'existing' &&
      authCookieSource !== 'legacy' &&
      authCookieSource !== 'none')
  ) {
    return null;
  }
  return {
    localStorage,
    cookiesImported,
    encryptedCookiesSkipped,
    cookiesFailed,
    authCookieSource,
    cacheDetected: value['cacheDetected'],
  };
}

function parseJson(value: string): unknown {
  try {
    return JSON.parse(value) as unknown;
  } catch {
    return undefined;
  }
}

async function loadLegacyDataFromTauri(): Promise<unknown> {
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke<unknown>('import_legacy_renderer_data');
}

export async function migrateLegacyRendererData({
  isTauri = isTauriRuntime,
  storage = localStorage,
  loadLegacyData = loadLegacyDataFromTauri,
}: LegacyRendererMigrationOptions = {}): Promise<LegacyRendererMigrationResult | null> {
  if (!isTauri) return null;
  const marker = storage.getItem(MIGRATION_MARKER);
  if (marker !== null) return null;
  if (storage.getItem('appVersion') !== null) {
    storage.setItem(MIGRATION_MARKER, 'skipped-existing-tauri-data');
    return null;
  }
  const writableKeys = new Set(
    MIGRATED_STORAGE_KEYS.filter(key => storage.getItem(key) === null)
  );

  try {
    const decoded = decodeNativeResult(await loadLegacyData());
    if (decoded === null) {
      console.warn('[migration] Invalid Electron renderer migration payload');
      return { status: 'retry-required' };
    }

    const migratedKeys: string[] = [];
    const failedKeys: MigratedStorageKey[] = [];
    let hadLegacyAccount = false;
    const rawData = decoded.localStorage['data'];
    if (writableKeys.has('data')) {
      const data = rawData === undefined ? undefined : parseJson(rawData);
      if (decoded.authCookieSource === 'existing') {
        storage.setItem(
          'data',
          JSON.stringify({
            ...defaultStorageState.data,
            loginMode: 'account',
          })
        );
        migratedKeys.push('data');
        if (rawData !== undefined && !isUnknownRecord(data)) {
          failedKeys.push('data');
        }
      } else if (isUnknownRecord(data)) {
        const decodedData = decodeDataState(data, defaultStorageState.data);
        hadLegacyAccount = decodedData.loginMode === 'account';
        storage.setItem(
          'data',
          JSON.stringify(
            hadLegacyAccount && decoded.authCookieSource === 'none'
              ? defaultStorageState.data
              : decodedData
          )
        );
        migratedKeys.push('data');
      } else if (decoded.authCookieSource === 'legacy') {
        storage.setItem(
          'data',
          JSON.stringify({
            ...defaultStorageState.data,
            loginMode: 'account',
          })
        );
        migratedKeys.push('data');
        if (rawData !== undefined) failedKeys.push('data');
      } else {
        if (rawData !== undefined) failedKeys.push('data');
      }
    }

    const rawLastfm = decoded.localStorage['lastfm'];
    if (rawLastfm !== undefined && writableKeys.has('lastfm')) {
      const lastfm = parseJson(rawLastfm);
      if (isUnknownRecord(lastfm)) {
        storage.setItem('lastfm', JSON.stringify(decodeLastfmState(lastfm)));
        migratedKeys.push('lastfm');
      } else {
        failedKeys.push('lastfm');
      }
    }

    const rawPlayer = decoded.localStorage['player'];
    if (rawPlayer !== undefined && writableKeys.has('player')) {
      const player = parseJson(rawPlayer);
      if (isUnknownRecord(player)) {
        storage.setItem('player', JSON.stringify(decodeStoredRecord(player)));
        migratedKeys.push('player');
      } else {
        failedKeys.push('player');
      }
    }

    const rawProgress = decoded.localStorage['playerCurrentTrackTime'];
    if (
      rawProgress !== undefined &&
      writableKeys.has('playerCurrentTrackTime')
    ) {
      const progress = Number(rawProgress);
      if (
        rawProgress.trim() !== '' &&
        Number.isFinite(progress) &&
        progress >= 0
      ) {
        storage.setItem('playerCurrentTrackTime', String(progress));
        migratedKeys.push('playerCurrentTrackTime');
      } else {
        failedKeys.push('playerCurrentTrackTime');
      }
    }

    const loginRequired =
      hadLegacyAccount && decoded.authCookieSource === 'none';
    const notice: LegacyMigrationNotice | null =
      failedKeys.length > 0
        ? 'partial-import'
        : loginRequired
        ? decoded.cacheDetected
          ? 'login-and-cache'
          : 'login-required'
        : decoded.cacheDetected
        ? 'cache-not-migrated'
        : migratedKeys.length > 0 || decoded.cookiesImported > 0
        ? 'complete'
        : null;
    storage.setItem(MIGRATION_MARKER, notice === null ? 'missing' : notice);
    return {
      status: 'completed',
      migratedKeys,
      failedKeys,
      cookiesImported: decoded.cookiesImported,
      notice,
    };
  } catch (error) {
    console.warn('[migration] Unable to import Electron renderer data', error);
    return { status: 'retry-required' };
  }
}
