import { describe, expect, test } from 'bun:test';
import { createPreciseSeekUpgrader } from '../src/utils/preciseSeekUpgrade';

/**
 * 用可手动推进的假依赖驱动编排器，直接覆盖 review 指出的三个竞态：
 * 转换期间切歌再拖拽、转换期间流式 seek 到 0、转换期间点暂停。
 */
function makeHarness({ sidecarUrl = '/precise-wav/1.wav' } = {}) {
  const state = {
    howler: { id: 'A' },
    trackId: 1,
    playing: true,
    pausePending: false,
    seekToken: 0,
  };
  const calls = { freeze: [], stream: [], apply: [], convert: [] };
  let releaseConvert;
  const upgrader = createPreciseSeekUpgrader({
    getSnapshot: () => ({ ...state }),
    readCachedFlac: async trackId => ({ bytes: `flac-${trackId}`, bitsPerSample: 24 }),
    convertViaSidecar: (trackId, bytes) =>
      new Promise(resolve => {
        calls.convert.push({ trackId, bytes });
        releaseConvert = () => resolve(sidecarUrl && `/precise-wav/${trackId}.wav`);
      }),
    convertInRenderer: async () => null,
    freezeAt: time => calls.freeze.push(time),
    seekStream: (time, sendMpris) => calls.stream.push({ time, sendMpris }),
    applyPreciseSource: (url, time, sendMpris, resume) =>
      calls.apply.push({ url, time, resume }),
  });
  // 模拟 Player.seek 的行为：任何显式 seek 都推进代际
  const seek = time => {
    state.seekToken += 1;
    upgrader.request(time, true);
  };
  const streamSeek = () => {
    state.seekToken += 1; // 非升级路径的 seek 只推进代际
  };
  const settle = async () => {
    // 先让 runOnce 推进到转换处装上 release，再放行，再等应用完成
    await new Promise(resolve => setTimeout(resolve, 0));
    releaseConvert?.();
    releaseConvert = null;
    await new Promise(resolve => setTimeout(resolve, 0));
  };
  return { state, calls, upgrader, seek, streamSeek, settle };
}

describe('精确 seek 升级编排器', () => {
  test('正常路径：冻结→sidecar 转换→按目标位置应用并恢复播放', async () => {
    const h = makeHarness();
    h.seek(95);
    await h.settle();
    expect(h.calls.freeze).toEqual([95]);
    expect(h.calls.apply).toEqual([
      { url: '/precise-wav/1.wav', time: 95, resume: true },
    ]);
    expect(h.calls.stream).toEqual([]);
  });

  test('竞态：转换期间切歌并拖拽新歌，旧任务作废、新歌用新快照重跑', async () => {
    const h = makeHarness();
    h.seek(100); // A 歌开始转换
    // 切到 B 歌（howler 替换 + trackId 变化），再拖 B
    h.state.howler = { id: 'B' };
    h.state.trackId = 2;
    h.seek(30);
    await h.settle(); // A 读完缓存即发现快照变化，提前放弃；B 接着转换
    await h.settle();
    // A 连转换都不浪费；B 以自己的 trackId 转换并应用到 30 秒
    expect(h.calls.apply).toEqual([
      { url: '/precise-wav/2.wav', time: 30, resume: true },
    ]);
    expect(h.calls.convert.map(c => c.trackId)).toEqual([2]);
  });

  test('竞态：转换期间流式 seek（如跳到 0），转换结束绝不回跳旧目标', async () => {
    const h = makeHarness();
    h.seek(100);
    h.streamSeek(); // seek(0) 走流式路径，只推进代际
    await h.settle();
    expect(h.calls.apply).toEqual([]);
    expect(h.calls.stream).toEqual([]); // 流式 seek 已自行生效，编排器不再插手
  });

  test('竞态：转换期间点了暂停（fade 未完成），切换后不得偷偷恢复播放', async () => {
    const h = makeHarness();
    h.seek(60);
    h.state.pausePending = true; // 暂停手势进行中
    await h.settle();
    expect(h.calls.apply).toEqual([
      { url: '/precise-wav/1.wav', time: 60, resume: false },
    ]);
  });

  test('连续拖拽合并：应用的是最后一次的目标', async () => {
    const h = makeHarness();
    h.seek(50);
    h.seek(80); // 第一次还在转换中
    await h.settle(); // 第一次返回 → 因代际变化被放弃，循环跑第二次
    await h.settle();
    expect(h.calls.apply).toEqual([
      { url: '/precise-wav/1.wav', time: 80, resume: true },
    ]);
  });

  test('缓存未写完时解冻并回退流式 seek', async () => {
    const state = { howler: {}, trackId: 9, playing: true, pausePending: false, seekToken: 1 };
    const stream = [];
    const upgrader = createPreciseSeekUpgrader({
      getSnapshot: () => ({ ...state }),
      readCachedFlac: async () => null,
      convertViaSidecar: async () => '/x.wav',
      convertInRenderer: async () => null,
      freezeAt: () => {},
      seekStream: (time, sendMpris) => stream.push({ time, sendMpris }),
      applyPreciseSource: () => {},
    });
    upgrader.request(77, false);
    await new Promise(resolve => setTimeout(resolve, 0));
    expect(stream).toEqual([{ time: 77, sendMpris: false }]);
  });

  test('sidecar 失败退回渲染进程转换，两者都失败才退流式', async () => {
    const state = { howler: {}, trackId: 5, playing: false, pausePending: false, seekToken: 1 };
    const applied = [];
    const stream = [];
    let rendererResult = 'blob:wav';
    const upgrader = createPreciseSeekUpgrader({
      getSnapshot: () => ({ ...state }),
      readCachedFlac: async () => ({ bytes: 'flac', bitsPerSample: 16 }),
      convertViaSidecar: async () => null,
      convertInRenderer: async () => rendererResult,
      freezeAt: () => {},
      seekStream: time => stream.push(time),
      applyPreciseSource: (url, time, sendMpris, resume) =>
        applied.push({ url, resume }),
    });
    upgrader.request(10, true);
    await new Promise(resolve => setTimeout(resolve, 0));
    expect(applied).toEqual([{ url: 'blob:wav', resume: false }]);

    rendererResult = null;
    upgrader.request(20, true);
    await new Promise(resolve => setTimeout(resolve, 0));
    expect(stream).toEqual([20]);
  });
});
