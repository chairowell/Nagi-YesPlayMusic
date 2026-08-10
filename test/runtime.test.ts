import { describe, expect, test } from 'bun:test';
import { resolveRuntime } from '../src/utils/runtime';

describe('桌面运行时识别', () => {
  test('Tauri 使用桌面路由、缓存和同源 API', () => {
    expect(resolveRuntime({ IS_TAURI: true })).toEqual({
      isTauri: true,
      isDesktop: true,
    });
  });

  test('普通 Web 仍保持原有行为', () => {
    expect(resolveRuntime({})).toEqual({
      isTauri: false,
      isDesktop: false,
    });
  });
});
