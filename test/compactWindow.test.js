import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import {
  buildCompactWindowTransitionFrame,
  COMPACT_RESIZE_SETTLE_MS,
  isCompactWindowPhysicalSize,
  loadCompactWindowMemory,
  rememberCompactWindowFrame,
} from '../src/services/compactWindow';

const app = readFileSync(new URL('../src/App.vue', import.meta.url), 'utf8');
const lyrics = readFileSync(
  new URL('../src/views/lyrics.vue', import.meta.url),
  'utf8'
);
const navbar = readFileSync(
  new URL('../src/components/Navbar.vue', import.meta.url),
  'utf8'
);
const compactWindow = readFileSync(
  new URL('../src/services/compactWindow.js', import.meta.url),
  'utf8'
);
const electronIpc = readFileSync(
  new URL('../src/electron/ipcMain.js', import.meta.url),
  'utf8'
);
const tauriMain = readFileSync(
  new URL('../src-tauri/src/main.rs', import.meta.url),
  'utf8'
);

test('小窗双击进入播放队列，中窗提供明确的返回入口', () => {
  expect(lyrics).toContain('@dblclick="handleMiniDoubleClick"');
  expect(lyrics).toContain("this.$emit('expand-compact-window')");
  expect(app).toContain("this.$router.push({ name: 'next' })");
  expect(navbar).toContain('title="回到播放栏 (Esc)"');
  expect(navbar).toContain("$emit('restore-compact-window')");
});

test('Retina 物理像素先换算成逻辑像素再判断小窗', () => {
  expect(isCompactWindowPhysicalSize({ width: 988, height: 508 }, 2)).toBe(
    true
  );
  expect(isCompactWindowPhysicalSize({ width: 1840, height: 1240 }, 2)).toBe(
    false
  );
});

test('两个桌面运行时和启动路径都拒绝恢复到屏幕外', () => {
  expect(compactWindow).toContain("invoke('restore_compact_window'");
  expect(electronIpc).toContain('hasReachableWindowArea');
  expect(tauriMain).toContain('ensure_main_window_reachable(&window)?;');
});

test('Bar 和浏览尺寸分别记忆，更新一档不会覆盖另一档', () => {
  const values = new Map();
  const storage = {
    getItem: key => values.get(key) ?? null,
    setItem: (key, value) => values.set(key, value),
  };

  rememberCompactWindowFrame({ x: 20, y: 30, width: 560, height: 72 }, storage);
  rememberCompactWindowFrame(
    { x: 100, y: 80, width: 1080, height: 700 },
    storage
  );
  rememberCompactWindowFrame({ x: 40, y: 50, width: 500, height: 64 }, storage);

  expect(loadCompactWindowMemory(storage)).toEqual({
    bar: { x: 40, y: 50, width: 500, height: 64 },
    browse: { x: 100, y: 80, width: 1080, height: 700 },
    lastMode: 'bar',
  });
  expect(COMPACT_RESIZE_SETTLE_MS).toBeGreaterThanOrEqual(200);
});

test('双屏切换档位时沿用当前屏位置，只恢复目标档位尺寸', () => {
  const currentOnRetina = { x: 5480, y: 220, width: 920, height: 620 };
  const rememberedBarOnExternal = { x: 180, y: 90, width: 520, height: 72 };

  expect(
    buildCompactWindowTransitionFrame(
      currentOnRetina,
      rememberedBarOnExternal
    )
  ).toEqual({ x: 5480, y: 220, width: 520, height: 72 });
});

test('中窗提供明确返回按钮和 Escape 快捷键', () => {
  expect(navbar).toContain('<span>回到播放栏</span>');
  expect(navbar).toContain('<kbd>Esc</kbd>');
  expect(app).toContain("e.code === 'Escape'");
  expect(app).toContain('this.restoreCompactWindow()');
  expect(electronIpc).not.toContain('compactWindowBounds');
  expect(electronIpc).toContain('applyCompactWindowFrame(target)');
});

test('重启时恢复最后使用的逻辑尺寸，不采用 Tauri 插件保存的物理像素', () => {
  const values = new Map();
  const storage = {
    getItem: key => values.get(key) ?? null,
    setItem: (key, value) => values.set(key, value),
  };

  rememberCompactWindowFrame(
    { x: 20, y: 30, width: 346, height: 177 },
    storage
  );
  rememberCompactWindowFrame(
    { x: 100, y: 80, width: 1060, height: 720 },
    storage
  );

  expect(loadCompactWindowMemory(storage)).toEqual({
    bar: { x: 20, y: 30, width: 346, height: 177 },
    browse: { x: 100, y: 80, width: 1060, height: 720 },
    lastMode: 'browse',
  });
  expect(compactWindow).toContain('restoreRememberedCompactWindowFrame');
  expect(app).toContain('compactWindowMemoryReady');
  expect(tauriMain).toContain('.skip_initial_state("main")');
});

test('Windows 和 Linux 从 mini bar 展开时先退出最大化且不记忆全屏尺寸', () => {
  const restoreCommand = tauriMain.slice(
    tauriMain.indexOf('fn restore_compact_window('),
    tauriMain.indexOf('fn create_tray(')
  );
  expect(restoreCommand.indexOf('window.unmaximize()')).toBeGreaterThan(-1);
  expect(restoreCommand.indexOf('window.unmaximize()')).toBeLessThan(
    restoreCommand.indexOf('.set_size(')
  );
  expect(compactWindow).toContain('window.isMaximized()');
  expect(compactWindow).toContain('window.isFullscreen()');
  expect(compactWindow).toContain('if (!frame || !snapshot.normal) return null');
  expect(electronIpc).toContain('maximized: win.isMaximized()');
  expect(electronIpc).toContain('fullscreen: win.isFullScreen()');
  expect(electronIpc).toContain("win.once('leave-full-screen', finish)");
  expect(compactWindow).toContain('await window.setFullscreen(false)');
  expect(electronIpc).toContain(
    '!win.isMaximized() &&\n      !win.isFullScreen()'
  );

  const doubleClick = lyrics.slice(
    lyrics.indexOf('handleMiniDoubleClick(event)'),
    lyrics.indexOf('updateMiniSeekPreview(event)')
  );
  expect(doubleClick).toContain('event.stopPropagation()');
});
