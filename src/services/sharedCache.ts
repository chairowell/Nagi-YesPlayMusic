import {
  createRemoteAudioSource,
  inferAudioFormatFromUrl,
  normalizeAudioFormat,
  sniffAudioFormat,
} from '@/utils/audioSource';
import {
  listTrackSourceMigrationIds,
  readTrackSourceForMigration,
} from '@/utils/db';
import type { AudioSource } from '@/utils/audioSource';
import type { Track } from '@/types/domain';
import type { SettingsState } from '@/types/persistence';
import type { TrackSourceRecord } from '@/utils/db';

const MIGRATION_MARKER = 'shared-cache-indexeddb-migration-v1';
const SUPPORTED_QUALITIES = new Set([128000, 192000, 320000, 350000, 999000]);
const SUPPORTED_CODECS = new Set(['mp3', 'flac', 'aac', 'm4a']);

type Fetcher = (
  input: RequestInfo | URL,
  init?: RequestInit
) => Promise<Response>;

interface StorageLike {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

export interface SharedCacheStatus {
  enabled: boolean;
  terminalCacheDetected: boolean;
}

export interface SharedCacheMigrationProgress {
  completed: number;
  total: number;
  imported: number;
  skipped: number;
}

interface SharedAudioProxyOptions {
  track: Track;
  quality: SettingsState['musicQuality'];
  source: string;
  format?: unknown;
  actualBitrate: number;
  cache: boolean;
  origin: string;
  provider?: string;
  excludedProviders?: string[];
}

interface SharedCacheImport {
  id: number;
  validatedTrackID: number;
  source: ArrayBuffer;
  bitRate: number;
  name: string;
  artist: string;
}

// Never rejects: every shared-cache entry point awaits it, so a rejected queue
// would poison the whole feature until the user touched the settings page.
let settingsQueue: Promise<void> = Promise.resolve();
// Tracks whether the renderer's toggle actually reached the sidecar. The sidecar
// keeps the switch in a process-local AtomicBool that defaults to false, so a
// failed sync means every proxy request would answer 409.
let sharedCacheHealthy = true;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

function responseError(response: Response, action: string): Error {
  return new Error(`${action} failed with HTTP ${response.status}`);
}

function trackArtist(track: Track): string {
  return track.ar?.[0]?.name ?? track.artists?.[0]?.name ?? 'Unknown';
}

function normalizeCodec(format: unknown, source?: string): string | null {
  const codec =
    normalizeAudioFormat(format) || inferAudioFormatFromUrl(source) || null;
  return codec && SUPPORTED_CODECS.has(codec) ? codec : null;
}

function sharedAudioPath(trackID: number, quality: number): string {
  const query = new URLSearchParams({ quality: String(quality) });
  return `/api/native/shared-cache/audio/${trackID}?${query}`;
}

export function normalizeSharedCacheQuality(
  quality: SettingsState['musicQuality']
): number {
  const normalized = quality === 'flac' ? 350000 : Number(quality);
  return SUPPORTED_QUALITIES.has(normalized) ? normalized : 320000;
}

export async function getSharedCacheStatus(
  fetcher: Fetcher = fetch
): Promise<SharedCacheStatus> {
  const response = await fetcher('/api/native/shared-cache/status');
  if (!response.ok) throw responseError(response, 'shared cache status');
  const value: unknown = await response.json();
  if (
    !isRecord(value) ||
    typeof value['enabled'] !== 'boolean' ||
    typeof value['terminalCacheDetected'] !== 'boolean'
  ) {
    throw new TypeError('shared cache status response is invalid');
  }
  return {
    enabled: value['enabled'],
    terminalCacheDetected: value['terminalCacheDetected'],
  };
}

async function configureSharedCache(
  enabled: boolean,
  fetcher: Fetcher
): Promise<void> {
  const response = await fetcher('/api/native/shared-cache/settings', {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ enabled }),
  });
  if (!response.ok) throw responseError(response, 'shared cache configuration');
}

export function syncSharedCacheSetting(
  enabled: boolean,
  fetcher: Fetcher = fetch
): Promise<void> {
  const sync = settingsQueue.then(async () => {
    try {
      await configureSharedCache(enabled, fetcher);
      sharedCacheHealthy = true;
    } catch (error) {
      sharedCacheHealthy = false;
      throw error;
    }
  });
  // Callers still see the rejection; the queue itself keeps a resolved promise.
  settingsQueue = sync.catch(() => undefined);
  return sync;
}

/**
 * Resolves once the pending settings sync settled. False means the sidecar is
 * not known to have the shared cache switched on, so callers must fall back to
 * direct playback instead of the same-origin proxy.
 */
export async function isSharedCacheHealthy(): Promise<boolean> {
  await settingsQueue;
  return sharedCacheHealthy;
}

/** Records that a shared-cache request was refused, e.g. after a sidecar restart. */
export function reportSharedCacheFailure(): void {
  sharedCacheHealthy = false;
}

/**
 * The single gate every audio-source resolver uses before handing playback a
 * same-origin proxy URL. Falling back to the direct URL keeps playback alive
 * when the sidecar never received (or lost) the enabled flag.
 */
export async function shouldUseSharedAudioProxy(
  enabled: boolean
): Promise<boolean> {
  return enabled && (await isSharedCacheHealthy());
}

export async function findSharedCachedAudio(
  trackID: number,
  quality: SettingsState['musicQuality'],
  fetcher: Fetcher = fetch
): Promise<AudioSource | null> {
  await settingsQueue;
  if (!sharedCacheHealthy) return null;
  const url = sharedAudioPath(trackID, normalizeSharedCacheQuality(quality));
  const response = await fetcher(url, { method: 'HEAD' });
  if (response.status === 404) return null;
  // 409 is the sidecar saying the shared cache is switched off on its side.
  if (response.status === 409) {
    reportSharedCacheFailure();
    return null;
  }
  if (!response.ok) throw responseError(response, 'shared cache lookup');
  const codec = normalizeCodec(response.headers.get('x-ypm-audio-codec'));
  return createRemoteAudioSource(url, {
    origin: 'cache',
    ...(codec === null ? {} : { format: codec }),
  });
}

export async function createSharedAudioProxy(
  options: SharedAudioProxyOptions
): Promise<AudioSource> {
  await settingsQueue;
  const quality = normalizeSharedCacheQuality(options.quality);
  const codec = normalizeCodec(options.format, options.source) ?? 'mp3';
  const params = new URLSearchParams({
    quality: String(quality),
    source: options.source,
    codec,
    actualBitrate: String(options.actualBitrate),
    cache: String(options.cache),
  });
  return createRemoteAudioSource(
    `/api/native/shared-cache/audio/${options.track.id}?${params}`,
    {
      origin: options.origin,
      format: codec,
      ...(options.provider === undefined ? {} : { provider: options.provider }),
      ...(options.excludedProviders === undefined
        ? {}
        : { excludedProviders: options.excludedProviders }),
    }
  );
}

export function isSharedAudioProxyURL(url: string): boolean {
  return url.startsWith('/api/native/shared-cache/audio/');
}

export async function prefetchSharedAudio(
  url: string,
  fetcher: Fetcher = fetch
): Promise<void> {
  await settingsQueue;
  const response = await fetcher(url);
  if (!response.ok) throw responseError(response, 'shared cache prefetch');
  await response.arrayBuffer();
}

export async function deleteSharedCachedAudio(
  trackID: number,
  quality: SettingsState['musicQuality'],
  fetcher: Fetcher = fetch
): Promise<void> {
  await settingsQueue;
  const response = await fetcher(
    sharedAudioPath(trackID, normalizeSharedCacheQuality(quality)),
    { method: 'DELETE' }
  );
  if (!response.ok) throw responseError(response, 'shared cache invalidation');
}

async function importSharedCacheRecord(
  record: SharedCacheImport,
  quality: SettingsState['musicQuality'],
  format: unknown,
  fetcher: Fetcher
): Promise<boolean> {
  if (
    record.id <= 0 ||
    record.validatedTrackID !== record.id ||
    !Number.isFinite(record.bitRate) ||
    record.bitRate <= 0
  ) {
    return false;
  }
  const codec =
    normalizeCodec(format) ?? normalizeCodec(sniffAudioFormat(record.source));
  if (codec === null) return false;

  const metadata = {
    trackId: record.id,
    quality: normalizeSharedCacheQuality(quality),
    codec,
    actualBitrate: Math.round(record.bitRate),
    name: record.name || 'Unknown',
    artist: record.artist || 'Unknown',
  };
  const body = new FormData();
  body.append('metadata', JSON.stringify(metadata));
  body.append(
    'audio',
    new Blob([record.source], { type: `audio/${codec}` }),
    `${record.id}.${codec}`
  );
  const response = await fetcher('/api/native/shared-cache/import', {
    method: 'POST',
    body,
  });
  if (!response.ok) throw responseError(response, 'shared cache import');
  return true;
}

export async function importTrackIntoSharedCache(
  track: Track,
  source: ArrayBuffer,
  bitRate: number,
  quality: SettingsState['musicQuality'],
  format: unknown,
  fetcher: Fetcher = fetch
): Promise<boolean> {
  await settingsQueue;
  return importSharedCacheRecord(
    {
      id: track.id,
      validatedTrackID: track.id,
      source,
      bitRate,
      name: track.name ?? 'Unknown',
      artist: trackArtist(track),
    },
    quality,
    format,
    fetcher
  );
}

export async function migrateIndexedDbTracksToSharedCache({
  quality,
  onProgress,
  fetcher = fetch,
  storage = localStorage,
}: {
  quality: SettingsState['musicQuality'];
  onProgress: (progress: SharedCacheMigrationProgress) => void;
  fetcher?: Fetcher;
  storage?: StorageLike;
}): Promise<SharedCacheMigrationProgress> {
  if (storage.getItem(MIGRATION_MARKER) === 'complete') {
    const completed = { completed: 0, total: 0, imported: 0, skipped: 0 };
    onProgress(completed);
    return completed;
  }

  const ids = await listTrackSourceMigrationIds();
  const progress: SharedCacheMigrationProgress = {
    completed: 0,
    total: ids.length,
    imported: 0,
    skipped: 0,
  };
  onProgress({ ...progress });
  for (const id of ids) {
    const record: TrackSourceRecord | undefined =
      await readTrackSourceForMigration(id);
    const imported =
      record === undefined
        ? false
        : await importSharedCacheRecord(
            record,
            quality,
            sniffAudioFormat(record.source),
            fetcher
          );
    progress.completed += 1;
    if (imported) progress.imported += 1;
    else progress.skipped += 1;
    onProgress({ ...progress });
  }
  storage.setItem(MIGRATION_MARKER, 'complete');
  return progress;
}
