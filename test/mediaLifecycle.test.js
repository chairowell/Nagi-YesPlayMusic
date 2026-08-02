import { describe, expect, test } from 'bun:test';
import {
  destroyMediaPlayer,
  stopInterval,
} from '../src/utils/mediaLifecycle';

describe('媒体组件生命周期', () => {
  test('离开 MV 页面时销毁播放器', () => {
    let destroyed = 0;
    destroyMediaPlayer({ destroy: () => (destroyed += 1) });
    expect(destroyed).toBe(1);
  });

  test('播放器尚未创建时也能安全退出', () => {
    expect(() => destroyMediaPlayer(null)).not.toThrow();
  });
});

describe('轮询生命周期', () => {
  test('组件卸载时停止已创建的轮询', () => {
    const cleared = [];
    stopInterval(42, timer => cleared.push(timer));
    expect(cleared).toEqual([42]);
  });

  test('未创建轮询时不会调用清理函数', () => {
    const cleared = [];
    stopInterval(null, timer => cleared.push(timer));
    expect(cleared).toEqual([]);
  });
});
