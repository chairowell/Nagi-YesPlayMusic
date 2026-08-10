import { describe, expect, test } from 'bun:test';
import {
  createUnblockMusicService,
  normalizeNeteaseTrack,
} from '../src/services/unblockMusic';
import type {
  Context,
  Executor,
  Song,
  SongSearchInformation,
} from '@unblockneteasemusic/rust-napi';

describe('UNM 桌面服务', () => {
  test('Tauri 使用统一的歌曲格式和音源过滤', async () => {
    const calls: Array<{
      sources: string[];
      song: Song;
      context: Context;
    }> = [];
    const executor: Executor = {
      list: () => ['qq', 'bilibili'],
      search: async (sources, song, context) => {
        calls.push({ sources, song, context });
        return { source: 'bilibili', identifier: 'video-a' };
      },
      retrieve: async matched => ({
        source: matched.source,
        url: 'https://example.bilivideo.com/a',
      }),
    };
    const unblock = createUnblockMusicService({
      executor,
      getBiliVideoFile: async () => 'YmFzZTY0',
      log: () => {},
    });

    const result = await unblock(
      ' QQ, invalid ',
      {
        id: 123,
        name: '测试歌曲',
        dt: 456,
        al: { id: 789, name: '测试专辑' },
        ar: [{ id: 101, name: '测试歌手' }],
      },
      { searchMode: 0 }
    );

    expect(calls).toEqual([
      {
        sources: ['qq'],
        song: {
          id: '123',
          name: '测试歌曲',
          duration: 456,
          album: { id: '789', name: '测试专辑' },
          artists: [{ id: '101', name: '测试歌手' }],
        },
        context: { searchMode: 0 },
      },
    ]);
    expect(result?.url).toBe('YmFzZTY0');
  });

  test('缺失专辑和歌手时仍能生成 UNM 输入', () => {
    expect(
      normalizeNeteaseTrack({ id: 123, name: '测试歌曲', dt: 456 })
    ).toEqual({
      id: '123',
      name: '测试歌曲',
      duration: 456,
      artists: [],
    });
  });

  test('单次检索失败沿用旧版行为并返回 null', async () => {
    const unblock = createUnblockMusicService({
      executor: {
        list: () => [],
        search: async () => {
          throw new Error('offline');
        },
        retrieve: async () => {
          throw new Error('无音源时不应进入 retrieve');
        },
      },
      log: () => {},
    });

    expect(await unblock(null, { id: 1 }, {})).toBeNull();
  });

  test('解码失败后可排除当前 provider，有界尝试下一个备用源', async () => {
    const calls: Array<{ sources: string[]; context: Context }> = [];
    const unblock = createUnblockMusicService({
      executor: {
        list: () => ['ytdl', 'bilibili', 'pyncm', 'kugou'],
        search: async (sources, _song, context) => {
          calls.push({ sources, context });
          const source = sources[0];
          if (!source) throw new Error('未提供可用音源');
          return { source, identifier: 'next' };
        },
        retrieve: async (matched: SongSearchInformation) => ({
          source: matched.source,
          url: 'https://example.com/audio.mp3',
        }),
      },
      log: () => {},
    });

    const result = await unblock(
      null,
      { id: 1 },
      {
        searchMode: 0,
        excludedSources: ['ytdl', 'bilibili'],
      }
    );

    expect(calls).toEqual([
      {
        sources: ['pyncm', 'kugou'],
        context: { searchMode: 0 },
      },
    ]);
    expect(result?.source).toBe('pyncm');
  });
});
