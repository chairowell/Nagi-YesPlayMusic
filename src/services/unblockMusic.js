import UNM from '@unblockneteasemusic/rust-napi';
import { Buffer } from 'node:buffer';

const DEFAULT_SOURCES = ['ytdl', 'bilibili', 'pyncm', 'kugou'];

function stringifyID(id) {
  return id == null ? id : String(id);
}

export function normalizeNeteaseTrack(ncmTrack) {
  return {
    id: stringifyID(ncmTrack.id),
    name: ncmTrack.name,
    duration: ncmTrack.dt,
    album: ncmTrack.al && {
      id: stringifyID(ncmTrack.al.id),
      name: ncmTrack.al.name,
    },
    artists: (ncmTrack.ar || []).map(({ id, name }) => ({
      id: stringifyID(id),
      name,
    })),
  };
}

function parseSourceString(executor, sourceString, log) {
  if (typeof sourceString !== 'string') return DEFAULT_SOURCES;
  const availableSources = executor.list();
  return sourceString
    .split(',')
    .map(source => source.trim().toLowerCase())
    .filter(source => {
      const available = availableSources.includes(source);
      if (!available) log(`[UNM] 忽略不支持的音源：${source}`);
      return available;
    });
}

export async function getBiliVideoFile(url) {
  const axios = await import('axios').then(module => module.default);
  const response = await axios.get(url, {
    headers: {
      Referer: 'https://www.bilibili.com/',
      'User-Agent': 'okhttp/3.4.1',
    },
    responseType: 'arraybuffer',
  });
  return Buffer.from(response.data).toString('base64');
}

export function createUnblockMusicService({
  executor = new UNM.Executor(),
  getBiliVideoFile: fetchBiliVideoFile = getBiliVideoFile,
  log = console.log,
} = {}) {
  return async (sourceListString, ncmTrack, context) => {
    const sourceList = parseSourceString(executor, sourceListString, log);
    const song = normalizeNeteaseTrack(ncmTrack);
    log(`[UNM] 使用音源：${sourceList.join(', ')}`);

    try {
      const matchedAudio = await executor.search(sourceList, song, context);
      const retrievedSong = await executor.retrieve(matchedAudio, context);
      if (retrievedSong.url.includes('bilivideo.com')) {
        retrievedSong.url = await fetchBiliVideoFile(retrievedSong.url);
      }
      return retrievedSong;
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      log(`[UNM] 检索失败：${message}`);
      return null;
    }
  };
}

export function listUnblockMusicSources() {
  return new UNM.Executor().list();
}
