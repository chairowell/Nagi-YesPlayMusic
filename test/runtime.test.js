import { describe, expect, test } from 'bun:test';
import { resolveRuntime } from '../src/utils/runtime';

describe('桌面运行时识别', () => {
  test('Electron 仍属于桌面端', () => {
    expect(resolveRuntime({ IS_ELECTRON: true })).toEqual({
      isElectron: true,
      isTauri: false,
      isDesktop: true,
    });
  });

  test('Tauri 使用桌面路由、缓存和同源 API，但不冒充 Electron', () => {
    expect(resolveRuntime({ IS_TAURI: true })).toEqual({
      isElectron: false,
      isTauri: true,
      isDesktop: true,
    });
  });

  test('普通 Web 仍保持原有行为', () => {
    expect(resolveRuntime({})).toEqual({
      isElectron: false,
      isTauri: false,
      isDesktop: false,
    });
  });
});
