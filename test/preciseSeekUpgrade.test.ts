import { describe, expect, test } from 'bun:test';
import { createPreciseSeekUpgrader } from '../src/utils/preciseSeekUpgrade';

interface HarnessOptions {
  sidecarUrl?: string | null;
}

interface HarnessState {
  howler: { id: string };
  trackId: number;
  playing: boolean;
  pausePending: boolean;
  seekToken: number;
}

interface ConversionCall {
  trackId: number;
  bytes: string;
}

interface StreamCall {
  time: number;
}

interface ApplyCall {
  url: string;
  time: number;
  resume: boolean;
}

// Drive race scenarios with manually controlled dependencies.
function makeHarness({
  sidecarUrl = '/precise-wav/1.wav',
}: HarnessOptions = {}) {
  const state: HarnessState = {
    howler: { id: 'A' },
    trackId: 1,
    playing: true,
    pausePending: false,
    seekToken: 0,
  };
  const calls: {
    freeze: number[];
    stream: StreamCall[];
    apply: ApplyCall[];
    convert: ConversionCall[];
  } = { freeze: [], stream: [], apply: [], convert: [] };
  const conversion: { release?: () => void } = {};
  const upgrader = createPreciseSeekUpgrader({
    getSnapshot: () => ({ ...state }),
    readCachedFlac: async trackId => ({
      bytes: `flac-${trackId}`,
      bitsPerSample: 24,
    }),
    convertViaSidecar: (trackId, bytes) =>
      new Promise(resolve => {
        calls.convert.push({ trackId, bytes });
        conversion.release = () =>
          resolve(sidecarUrl ? `/precise-wav/${trackId}.wav` : null);
      }),
    convertInRenderer: async () => null,
    freezeAt: time => calls.freeze.push(time),
    seekStream: time => calls.stream.push({ time }),
    applyPreciseSource: (url, time, resume) =>
      calls.apply.push({ url, time, resume }),
  });
  // Every explicit seek advances the generation.
  const seek = (time: number): void => {
    state.seekToken += 1;
    upgrader.request(time);
  };
  const streamSeek = () => {
    state.seekToken += 1;
  };
  const settle = async () => {
    // Wait for conversion to expose release before resolving it.
    await new Promise(resolve => setTimeout(resolve, 0));
    conversion.release?.();
    delete conversion.release;
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
    h.seek(100);
    // Replace the player with track B, then seek B.
    h.state.howler = { id: 'B' };
    h.state.trackId = 2;
    h.seek(30);
    await h.settle();
    await h.settle();
    // A aborts and B applies its own conversion at 30 seconds.
    expect(h.calls.apply).toEqual([
      { url: '/precise-wav/2.wav', time: 30, resume: true },
    ]);
    expect(h.calls.convert.map(c => c.trackId)).toEqual([2]);
  });

  test('竞态：转换期间流式 seek（如跳到 0），转换结束绝不回跳旧目标', async () => {
    const h = makeHarness();
    h.seek(100);
    h.streamSeek();
    await h.settle();
    expect(h.calls.apply).toEqual([]);
    expect(h.calls.stream).toEqual([]);
  });

  test('竞态：转换期间点了暂停（fade 未完成），切换后不得偷偷恢复播放', async () => {
    const h = makeHarness();
    h.seek(60);
    h.state.pausePending = true;
    await h.settle();
    expect(h.calls.apply).toEqual([
      { url: '/precise-wav/1.wav', time: 60, resume: false },
    ]);
  });

  test('连续拖拽合并：应用的是最后一次的目标', async () => {
    const h = makeHarness();
    h.seek(50);
    h.seek(80);
    await h.settle();
    await h.settle();
    expect(h.calls.apply).toEqual([
      { url: '/precise-wav/1.wav', time: 80, resume: true },
    ]);
  });

  test('缓存未写完时解冻并回退流式 seek', async () => {
    const state = {
      howler: {},
      trackId: 9,
      playing: true,
      pausePending: false,
      seekToken: 1,
    };
    const stream: StreamCall[] = [];
    const upgrader = createPreciseSeekUpgrader({
      getSnapshot: () => ({ ...state }),
      readCachedFlac: async () => null,
      convertViaSidecar: async () => '/x.wav',
      convertInRenderer: async () => null,
      freezeAt: () => {},
      seekStream: time => stream.push({ time }),
      applyPreciseSource: () => {},
    });
    upgrader.request(77);
    await new Promise(resolve => setTimeout(resolve, 0));
    expect(stream).toEqual([{ time: 77 }]);
  });

  test('sidecar 失败退回渲染进程转换，两者都失败才退流式', async () => {
    const state = {
      howler: {},
      trackId: 5,
      playing: false,
      pausePending: false,
      seekToken: 1,
    };
    const applied: Array<{ url: string; resume: boolean }> = [];
    const stream: number[] = [];
    let rendererResult: string | null = 'blob:wav';
    const upgrader = createPreciseSeekUpgrader({
      getSnapshot: () => ({ ...state }),
      readCachedFlac: async () => ({ bytes: 'flac', bitsPerSample: 16 }),
      convertViaSidecar: async () => null,
      convertInRenderer: async () => rendererResult,
      freezeAt: () => {},
      seekStream: time => stream.push(time),
      applyPreciseSource: (url, _time, resume) => applied.push({ url, resume }),
    });
    upgrader.request(10);
    await new Promise(resolve => setTimeout(resolve, 0));
    expect(applied).toEqual([{ url: 'blob:wav', resume: false }]);

    rendererResult = null;
    upgrader.request(20);
    await new Promise(resolve => setTimeout(resolve, 0));
    expect(stream).toEqual([20]);
  });
});
