import { describe, expect, test } from 'bun:test';
import { destroyMediaPlayer } from '../src/utils/mediaLifecycle';

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
