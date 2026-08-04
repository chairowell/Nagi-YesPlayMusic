import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import {
  createNextTrackPrefetcher,
  warmTrackArtwork,
} from '../src/utils/trackPrefetch';
import {
  ARTWORK_SIZE,
  PREFETCHED_ARTWORK_SIZES,
  UPCOMING_ARTWORK_COUNT,
} from '../src/utils/artwork';
import { getUpcomingTrackIDs } from '../src/utils/playerQueue';

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
    // 慢的根因是无损音频占满带宽，预热要是抢优先级只会更糟
    expect(images.map(image => image.fetchPriority)).toEqual(
      urls.map(() => 'low')
    );
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

  test('封面预热提前备好后面几首，而不是只备下一首', () => {
    // 只备一首时，下一首的预热常常还没跑完人就切走了（实测封面仍要 0.75~1.6s）
    expect(UPCOMING_ARTWORK_COUNT).toBeGreaterThan(1);
    expect(getUpcomingTrackIDs([1, 2, 3, 4, 5], 0, 1, false, 3)).toEqual([
      2, 3, 4,
    ]);
    // 队列到头就停，不靠 undefined 撑满数量
    expect(getUpcomingTrackIDs([1, 2, 3], 1, 1, false, 3)).toEqual([3]);
    // 允许绕回时不重复预热，绕到正在播的这首就停
    expect(getUpcomingTrackIDs([1, 2], 1, 1, true, 3)).toEqual([1]);
    expect(getUpcomingTrackIDs([7], 0, 1, true, 3)).toEqual([]);
    // 倒序播放时往反方向备
    expect(getUpcomingTrackIDs([1, 2, 3, 4], 3, -1, false, 2)).toEqual([3, 2]);
  });

  test('封面预热挂在 commitTrack 上，不排在音源解析后面', () => {
    const playerSource = readFileSync(
      new URL('../src/utils/Player.js', import.meta.url),
      'utf8'
    );
    const commitTrack = playerSource.slice(
      playerSource.indexOf('commitTrack: track => {'),
      playerSource.indexOf('loadSource: track =>')
    );
    // commitSource 要等 song/url（实测 1.9 秒），封面预热等不起
    expect(commitTrack).toContain('this._warmUpcomingArtwork();');
    // 只读本地详情：为一张封面再发一轮 song/detail 反而是在抢带宽
    const warm = playerSource.slice(
      playerSource.indexOf('_warmUpcomingArtwork() {'),
      playerSource.indexOf('_prefetchNextTrack() {')
    );
    expect(warm).toContain('getTrackDetailFromCache');
    expect(warm).not.toContain('getTrackDetail(');
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
