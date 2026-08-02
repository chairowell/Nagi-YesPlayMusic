import { describe, expect, test } from 'bun:test';
import {
  createUnblockMusicService,
  normalizeNeteaseTrack,
} from '../src/services/unblockMusic';

describe('UNM 桌面服务', () => {
  test('Electron 与 Tauri 共用同一套歌曲格式和音源过滤', async () => {
    const calls = [];
    const executor = {
      list: () => ['qq', 'bilibili'],
      search: async (sources, song, context) => {
        calls.push({ sources, song, context });
        return { source: 'bilibili', url: 'https://example.bilivideo.com/a' };
      },
      retrieve: async matched => ({ ...matched }),
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
    expect(result.url).toBe('YmFzZTY0');
  });

  test('缺失专辑和歌手时仍能生成 UNM 输入', () => {
    expect(
      normalizeNeteaseTrack({ id: 123, name: '测试歌曲', dt: 456 })
    ).toEqual({
      id: '123',
      name: '测试歌曲',
      duration: 456,
      album: undefined,
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
      },
      log: () => {},
    });

    expect(await unblock(null, { id: 1 }, {})).toBeNull();
  });

  test('解码失败后可排除当前 provider，有界尝试下一个备用源', async () => {
    const calls = [];
    const unblock = createUnblockMusicService({
      executor: {
        list: () => ['ytdl', 'bilibili', 'pyncm', 'kugou'],
        search: async (sources, _song, context) => {
          calls.push({ sources, context });
          return { source: sources[0], identifier: 'next' };
        },
        retrieve: async matched => ({
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
    expect(result.source).toBe('pyncm');
  });
});
