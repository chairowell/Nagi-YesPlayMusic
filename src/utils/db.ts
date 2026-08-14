import axios from 'axios';
import Dexie from 'dexie';
import type { Table } from 'dexie';
import { getAppStore, getAppStoreIfReady } from '@/stores/accessor';
import { isTrustedTrackSource } from '@/utils/audioCacheIntegrity';
import { createKeyedTaskPool, sumTrackSourceStats } from '@/utils/cacheStats';
import { isCacheLimitExceeded } from '@/utils/cachePolicy';
import { isDesktopRuntime } from '@/utils/runtime';
import type { Track, TrackPrivilege } from '@/types/domain';
import type { SettingsState } from '@/types/persistence';
import type { LyricsResponse } from '@/utils/lyrics';
import type { TrackCollectionResponse } from '@/api/types';
// import pkg from "../../package.json";

export interface TrackSourceRecord {
  id: number;
  validatedTrackID: number;
  source: ArrayBuffer;
  bitRate: number;
  from: string;
  name: string;
  artist: string;
  createTime: number;
}

interface TrackDetailRecord {
  id: number;
  detail: Track;
  privileges?: TrackPrivilege;
  updateTime: number;
}

interface LyricRecord {
  id: number;
  lyrics: LyricsResponse;
  updateTime: number;
}

interface AlbumRecord {
  id: number;
  album: TrackCollectionResponse;
  updateTime: number;
}

class YesPlayMusicDatabase extends Dexie {
  trackSources!: Table<TrackSourceRecord, number>;
  trackDetail!: Table<TrackDetailRecord, number>;
  lyric!: Table<LyricRecord, number>;
  album!: Table<AlbumRecord, number>;

  constructor() {
    super('yesplaymusic');
  }
}

const db = new YesPlayMusicDatabase();

db.version(4).stores({
  trackDetail: '&id, updateTime',
  lyric: '&id, updateTime',
  album: '&id, updateTime',
});

db.version(3)
  .stores({
    trackSources: '&id, createTime',
  })
  .upgrade(tx =>
    tx
      .table('trackSources')
      .toCollection()
      .modify(
        (track: TrackSourceRecord) =>
          !track.createTime && (track.createTime = new Date().getTime())
      )
  );

db.version(1).stores({
  trackSources: '&id',
});

let tracksCacheBytes = 0;
const runTrackSourceCacheOnce = createKeyedTaskPool();

// Wait until settings are available.
async function waitForSettingsReady(
  timeoutMs = 5000
): Promise<SettingsState | null> {
  const interval = 100;
  const maxTries = Math.ceil(timeoutMs / interval);
  let tries = 0;
  while (getAppStoreIfReady() === null && tries < maxTries) {
    await new Promise(resolve => setTimeout(resolve, interval));
    tries++;
  }
  return getAppStoreIfReady()?.settings ?? null;
}

// Measure existing entries before enforcing the cache limit.
async function initTracksCacheBytes(): Promise<void> {
  if (!isDesktopRuntime) return;
  try {
    await waitForSettingsReady();
    const stats = await sumTrackSourceStats(visitor =>
      db.trackSources.each(visitor)
    );
    tracksCacheBytes = stats.bytes;
    console.debug(
      '[debug][db.js] initTracksCacheBytes, total bytes:',
      tracksCacheBytes
    );
    void trimTrackSourceCache();
  } catch (err) {
    console.debug('[debug][db.js] initTracksCacheBytes failed', err);
  }
}

// Start initialization at module load.
void initTracksCacheBytes();

export async function trimTrackSourceCache(): Promise<void> {
  try {
    while (
      isCacheLimitExceeded(tracksCacheBytes, getAppStore().settings.cacheLimit)
    ) {
      const delCache = await db.trackSources.orderBy('createTime').first();
      if (!delCache) {
        tracksCacheBytes = 0;
        return;
      }

      await db.trackSources.delete(delCache.id);
      tracksCacheBytes = Math.max(
        0,
        tracksCacheBytes - (delCache.source?.byteLength || 0)
      );
      console.debug(
        `[debug][db.js] deleteExcessCacheSuccess, track: ${
          delCache.name
        }, size: ${
          delCache.source?.byteLength || 0
        }, cacheSize:${tracksCacheBytes}`
      );
    }
  } catch (error) {
    console.debug('[debug][db.js] deleteExcessCacheFailed', error);
  }
}

export function cacheTrackSource(
  trackInfo: Track,
  url: string,
  bitRate: number,
  from = 'netease'
) {
  if (!isDesktopRuntime) return;
  return runTrackSourceCacheOnce(trackInfo.id, async () => {
    // Cache hits return through the read path and must not be counted twice.
    if (await hasTrackSource(trackInfo.id)) return null;

    const name = trackInfo.name ?? 'Unknown';
    const artist =
      (trackInfo.ar && trackInfo.ar[0]?.name) ||
      (trackInfo.artists && trackInfo.artists[0]?.name) ||
      'Unknown';
    let cover = trackInfo.al?.picUrl ?? trackInfo.album?.picUrl ?? '';
    if (cover && cover.slice(0, 5) !== 'https') {
      cover = 'https' + cover.slice(4);
    }
    if (cover) {
      void axios.get(`${cover}?param=512y512`);
      void axios.get(`${cover}?param=224y224`);
      void axios.get(`${cover}?param=1024y1024`);
    }

    const response = await axios.get<ArrayBuffer>(url, {
      responseType: 'arraybuffer',
    });
    await db.trackSources.put({
      id: trackInfo.id,
      validatedTrackID: Number(trackInfo.id),
      source: response.data,
      bitRate,
      from,
      name,
      artist,
      createTime: new Date().getTime(),
    });
    console.debug(`[debug][db.js] cached track 👉 ${name} by ${artist}`);
    tracksCacheBytes += response.data.byteLength;
    await trimTrackSourceCache();
    return { trackID: trackInfo.id, source: response.data, bitRate };
  });
}

export async function getTrackSource(
  id: number | string
): Promise<TrackSourceRecord | null> {
  const trackID = Number(id);
  const track = await db.trackSources.get(trackID);
  if (!track) return null;
  if (!isTrustedTrackSource(track, trackID)) {
    // Legacy entries lack track IDs and cannot prove their source URL is safe.
    await deleteTrackSource(trackID);
    console.warn(`[Player] 已丢弃未校验的历史音频缓存：${track.name}`);
    return null;
  }
  console.debug(
    `[debug][db.js] get track from cache 👉 ${track.name} by ${track.artist}`
  );
  return track;
}

export async function deleteTrackSource(id: number | string): Promise<boolean> {
  const trackID = Number(id);
  const track = await db.trackSources.get(trackID);
  if (!track) return false;
  await db.trackSources.delete(trackID);
  tracksCacheBytes = Math.max(
    0,
    tracksCacheBytes - (track.source?.byteLength || 0)
  );
  return true;
}

export function hasTrackSource(id: number | string): Promise<boolean> {
  return db.trackSources
    .where('id')
    .equals(Number(id))
    .count()
    .then(count => count > 0);
}

export function listTrackSourceMigrationIds(): Promise<number[]> {
  return db.trackSources
    .toCollection()
    .primaryKeys()
    .then(keys => keys.filter((key): key is number => typeof key === 'number'));
}

export function readTrackSourceForMigration(
  id: number
): Promise<TrackSourceRecord | undefined> {
  return db.trackSources.get(id);
}

export function cacheTrackDetail(
  track: Track,
  privileges?: TrackPrivilege
): void {
  void db.trackDetail.put({
    id: track.id,
    detail: track,
    ...(privileges === undefined ? {} : { privileges }),
    updateTime: new Date().getTime(),
  });
}

export function getTrackDetailFromCache(
  ids: string[]
): Promise<TrackCollectionResponse | undefined> {
  return db.trackDetail
    .filter(track => {
      return ids.includes(String(track.id));
    })
    .toArray()
    .then(tracks => {
      const result: TrackCollectionResponse = { songs: [], privileges: [] };
      ids.forEach(id => {
        const one = tracks.find(t => String(t.id) === id);
        if (one) {
          result.songs.push(one.detail);
          result.privileges?.push(one.privileges ?? { id: one.id });
        }
      });
      if (result.songs.length !== ids.length) {
        return undefined;
      }
      return result;
    });
}

export function cacheLyric(id: number, lyrics: LyricsResponse): void {
  void db.lyric.put({
    id,
    lyrics,
    updateTime: new Date().getTime(),
  });
}

export function getLyricFromCache(
  id: number
): Promise<LyricsResponse | undefined> {
  return db.lyric.get(Number(id)).then(result => {
    if (!result) return undefined;
    return result.lyrics;
  });
}

export function cacheAlbum(id: number, album: TrackCollectionResponse): void {
  void db.album.put({
    id: Number(id),
    album,
    updateTime: new Date().getTime(),
  });
}

export function getAlbumFromCache(
  id: number
): Promise<TrackCollectionResponse | undefined> {
  return db.album.get(Number(id)).then(result => {
    if (!result) return undefined;
    return result.album;
  });
}

export function countDBSize(): Promise<{ bytes: number; length: number }> {
  return sumTrackSourceStats(visitor => db.trackSources.each(visitor)).then(
    res => {
      tracksCacheBytes = res.bytes;
      console.debug(
        `[debug][db.js] load tracksCacheBytes: ${tracksCacheBytes}`
      );
      return res;
    }
  );
}

export async function clearTrackSourceCache(): Promise<void> {
  await db.trackSources.clear();
  tracksCacheBytes = 0;
}
