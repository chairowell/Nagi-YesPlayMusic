import axios from 'axios';
import Dexie from 'dexie';
import store from '@/store';
import { isTrustedTrackSource } from '@/utils/audioCacheIntegrity';
import { createKeyedTaskPool, sumTrackSourceStats } from '@/utils/cacheStats';
import { isCacheLimitExceeded } from '@/utils/cachePolicy';
import { isDesktopRuntime } from '@/utils/runtime';
// import pkg from "../../package.json";

const db = new Dexie('yesplaymusic');

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
        track => !track.createTime && (track.createTime = new Date().getTime())
      )
  );

db.version(1).stores({
  trackSources: '&id',
});

let tracksCacheBytes = 0;
const runTrackSourceCacheOnce = createKeyedTaskPool();

// 等待 settings 可用
async function waitForSettingsReady(timeoutMs = 5000) {
  const interval = 100;
  const maxTries = Math.ceil(timeoutMs / interval);
  let tries = 0;
  while (
    (store.state == null ||
      store.state.settings == null ||
      store.state.settings.cacheLimit === undefined) &&
    tries < maxTries
  ) {
    await new Promise(resolve => setTimeout(resolve, interval));
    tries++;
  }
  return store.state && store.state.settings;
}

// 初始化现有缓存总大小，确保应用启动时能正确判断并清理超限缓存
async function initTracksCacheBytes() {
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
    trimTrackSourceCache();
  } catch (err) {
    console.debug('[debug][db.js] initTracksCacheBytes failed', err);
  }
}

// 模块加载时触发初始化
initTracksCacheBytes();

export async function trimTrackSourceCache() {
  try {
    while (
      isCacheLimitExceeded(tracksCacheBytes, store.state.settings.cacheLimit)
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

export function cacheTrackSource(trackInfo, url, bitRate, from = 'netease') {
  if (!isDesktopRuntime) return;
  return runTrackSourceCacheOnce(trackInfo.id, async () => {
    // 正在播放的缓存命中会从读取路径返回，不应再次覆盖并重复累计大小。
    if (await hasTrackSource(trackInfo.id)) return null;

    const name = trackInfo.name;
    const artist =
      (trackInfo.ar && trackInfo.ar[0]?.name) ||
      (trackInfo.artists && trackInfo.artists[0]?.name) ||
      'Unknown';
    let cover = trackInfo.al.picUrl;
    if (cover.slice(0, 5) !== 'https') {
      cover = 'https' + cover.slice(4);
    }
    axios.get(`${cover}?param=512y512`);
    axios.get(`${cover}?param=224y224`);
    axios.get(`${cover}?param=1024y1024`);

    const response = await axios.get(url, {
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

export async function getTrackSource(id) {
  const trackID = Number(id);
  const track = await db.trackSources.get(trackID);
  if (!track) return null;
  if (!isTrustedTrackSource(track, trackID)) {
    // 老缓存没有记录音源响应对应的歌曲 ID，无法排除曾被路径级 API 缓存串台。
    await deleteTrackSource(trackID);
    console.warn(`[Player] 已丢弃未校验的历史音频缓存：${track.name}`);
    return null;
  }
  console.debug(
    `[debug][db.js] get track from cache 👉 ${track.name} by ${track.artist}`
  );
  return track;
}

export async function deleteTrackSource(id) {
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

export function hasTrackSource(id) {
  return db.trackSources
    .where('id')
    .equals(Number(id))
    .count()
    .then(count => count > 0);
}

export function cacheTrackDetail(track, privileges) {
  db.trackDetail.put({
    id: track.id,
    detail: track,
    privileges: privileges,
    updateTime: new Date().getTime(),
  });
}

export function getTrackDetailFromCache(ids) {
  return db.trackDetail
    .filter(track => {
      return ids.includes(String(track.id));
    })
    .toArray()
    .then(tracks => {
      const result = { songs: [], privileges: [] };
      ids.map(id => {
        const one = tracks.find(t => String(t.id) === id);
        result.songs.push(one?.detail);
        result.privileges.push(one?.privileges);
      });
      if (result.songs.includes(undefined)) {
        return undefined;
      }
      return result;
    });
}

export function cacheLyric(id, lyrics) {
  db.lyric.put({
    id,
    lyrics,
    updateTime: new Date().getTime(),
  });
}

export function getLyricFromCache(id) {
  return db.lyric.get(Number(id)).then(result => {
    if (!result) return undefined;
    return result.lyrics;
  });
}

export function cacheAlbum(id, album) {
  db.album.put({
    id: Number(id),
    album,
    updateTime: new Date().getTime(),
  });
}

export function getAlbumFromCache(id) {
  return db.album.get(Number(id)).then(result => {
    if (!result) return undefined;
    return result.album;
  });
}

export function countDBSize() {
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

export async function clearTrackSourceCache() {
  await db.trackSources.clear();
  tracksCacheBytes = 0;
}
