import UNM from '@unblockneteasemusic/rust-napi';
import { Buffer } from 'node:buffer';
import type {
  Context,
  Executor,
  RetrievedSongInfo,
  SearchMode,
  Song,
} from '@unblockneteasemusic/rust-napi';
import type { Track } from '@/types/domain';

const DEFAULT_SOURCES = ['ytdl', 'bilibili', 'pyncm', 'kugou'];

interface UnblockContextInput extends Record<string, unknown> {
  excludedSources?: unknown;
  proxyUri?: unknown;
  enableFlac?: unknown;
  searchMode?: unknown;
  config?: unknown;
}

interface UnblockServiceOptions {
  executor?: Executor;
  getBiliVideoFile?: (url: string) => Promise<string>;
  log?: (message: string) => void;
}

export function normalizeNeteaseTrack(ncmTrack: Track): Song {
  return {
    id: String(ncmTrack.id),
    name: ncmTrack.name ?? '',
    ...(ncmTrack.dt === undefined ? {} : { duration: ncmTrack.dt }),
    ...(ncmTrack.al === undefined
      ? {}
      : {
          album: {
            id: String(ncmTrack.al.id),
            name: ncmTrack.al.name ?? '',
          },
        }),
    artists: (ncmTrack.ar || []).map(({ id, name }) => ({
      id: String(id),
      name: name ?? '',
    })),
  };
}

function parseSourceString(
  executor: Executor,
  sourceString: unknown,
  log: (message: string) => void
): string[] {
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

export async function getBiliVideoFile(url: string): Promise<string> {
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
}: UnblockServiceOptions = {}) {
  return async (
    sourceListString: unknown,
    ncmTrack: Track,
    context: UnblockContextInput = {}
  ): Promise<RetrievedSongInfo | null> => {
    const excludedSources = Array.isArray(context.excludedSources)
      ? context.excludedSources
      : [];
    const unmContext: Context = {
      ...(typeof context.proxyUri === 'string'
        ? { proxyUri: context.proxyUri }
        : {}),
      ...(typeof context.enableFlac === 'boolean'
        ? { enableFlac: context.enableFlac }
        : {}),
      ...(typeof context.searchMode === 'number'
        ? { searchMode: context.searchMode as SearchMode }
        : {}),
      ...(typeof context.config === 'object' && context.config !== null
        ? {
            config: Object.fromEntries(
              Object.entries(context.config).filter(
                (entry): entry is [string, string] =>
                  typeof entry[1] === 'string'
              )
            ),
          }
        : {}),
    };
    const excluded = new Set(
      excludedSources.map(source => String(source).toLowerCase())
    );
    const sourceList = parseSourceString(
      executor,
      sourceListString,
      log
    ).filter(source => !excluded.has(source));
    const song = normalizeNeteaseTrack(ncmTrack);
    log(`[UNM] 使用音源：${sourceList.join(', ')}`);

    try {
      if (!sourceList.length) return null;
      const matchedAudio = await executor.search(sourceList, song, unmContext);
      const retrievedSong = await executor.retrieve(matchedAudio, unmContext);
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

export function listUnblockMusicSources(): string[] {
  return new UNM.Executor().list();
}
