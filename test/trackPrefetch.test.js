import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import {
  createNextTrackPrefetcher,
  warmTrackArtwork,
} from '../src/utils/trackPrefetch';
import {
  ARTWORK_SIZE,
  PREFETCHED_ARTWORK_SIZES,
} from '../src/utils/artwork';

describe('下一首轻量预取', () => {
  test('同一目标去重，队列变化后淘汰旧响应', async () => {
    const resolvers = new Map();
    const loadedLyrics = [];
    const warmedTracks = [];
    const prefetcher = createNextTrackPrefetcher({
      loadTrack: id =>
        new Promise(resolve => {
          resolvers.set(id, resolve);
        }),
      loadLyric: id => loadedLyrics.push(id),
      warmArtwork: track => warmedTracks.push(track.id),
      cacheAudio: () => {
        throw new Error('未开启音频缓存时不应调用');
      },
    });

    const first = prefetcher.prefetch(101);
    await Promise.resolve();
    expect(prefetcher.prefetch(101)).toBe(first);
    const second = prefetcher.prefetch(202);
    await Promise.resolve();

    resolvers.get(101)({ id: 101, al: { picUrl: 'http://example/1.jpg' } });
    resolvers.get(202)({ id: 202, al: { picUrl: 'http://example/2.jpg' } });
    await Promise.all([first, second]);

    expect(loadedLyrics).toEqual([202]);
    expect(warmedTracks).toEqual([202]);
  });

  test('只为仍然有效的目标执行可选音频缓存', async () => {
    const cachedAudio = [];
    const prefetcher = createNextTrackPrefetcher({
      loadTrack: async id => ({ id, al: { picUrl: '' } }),
      loadLyric: async () => {},
      warmArtwork: () => {},
      cacheAudio: async track => cachedAudio.push(track.id),
    });

    await prefetcher.prefetch(303, { includeAudio: true });

    expect(cachedAudio).toEqual([303]);
  });

  test('封面预热使用 HTTPS，并只保留实际界面需要的尺寸', () => {
    const images = [];
    const urls = warmTrackArtwork(
      { al: { picUrl: 'http://p1.music.126.net/cover.jpg' } },
      () => {
        const image = {};
        images.push(image);
        return image;
      }
    );

    expect(urls).toEqual([
      'https://p1.music.126.net/cover.jpg?param=128y128',
      'https://p1.music.126.net/cover.jpg?param=224y224',
      'https://p1.music.126.net/cover.jpg?param=512y512',
    ]);
    expect(images.map(image => image.src)).toEqual(urls);
  });

  test('预取的尺寸必须覆盖迷你播放条实际要的那一档', () => {
    // 尺寸是缓存键的一部分：预取 224、界面要 128，等于预取了没人要的图。
    // 迷你条以前正是这样（预取 224/512、自己要 1024），封面每次都得现下。
    expect(PREFETCHED_ARTWORK_SIZES).toContain(ARTWORK_SIZE.miniPlayer);

    const lyricsSource = readFileSync(
      new URL('../src/views/lyrics.vue', import.meta.url),
      'utf8'
    );
    // 视图不许再写死数字，只能引用同一份常量，否则改一处漏一处
    expect(lyricsSource).toContain('ARTWORK_SIZE.miniPlayer');
    expect(lyricsSource).toContain('ARTWORK_SIZE.lyricsBackground');
    expect(lyricsSource).not.toMatch(/buildArtworkURL\([^)]*,\s*\d+\s*\)/);
  });

  test('Player 通过统一的下一首选择器决定预取目标', () => {
    const playerSource = readFileSync(
      new URL('../src/utils/Player.js', import.meta.url),
      'utf8'
    );

    expect(playerSource).toContain(': this._getNextTrack()[0]');
    expect(playerSource).toContain('this._nextTrackPrefetcher.prefetch(');
  });
});
