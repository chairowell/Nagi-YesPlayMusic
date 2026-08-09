import { electronRenderer } from '@/services/desktopTransport';
import { isTauriRuntime } from '@/utils/runtime';
import { isMiniWindowSize } from '@/utils/miniWindow';

export const COMPACT_EXPANDED_SIZE = Object.freeze({ width: 920, height: 620 });
export const COMPACT_RESIZE_SETTLE_MS = 250;
export const COMPACT_WINDOW_MEMORY_KEY = 'compactWindowFrames.v1';

const MIN_WINDOW_SIZE = Object.freeze({ width: 300, height: 48 });
const MAX_WINDOW_EDGE = 8192;
let compactWindowTransitioning = false;

function browserStorage() {
  try {
    return typeof window === 'undefined' ? null : window.localStorage;
  } catch (_) {
    return null;
  }
}

function normalizeCompactWindowFrame(frame) {
  const width = Number(frame?.width);
  const height = Number(frame?.height);
  if (
    !Number.isFinite(width) ||
    !Number.isFinite(height) ||
    width < MIN_WINDOW_SIZE.width ||
    height < MIN_WINDOW_SIZE.height ||
    width > MAX_WINDOW_EDGE ||
    height > MAX_WINDOW_EDGE
  ) {
    return null;
  }
  const rawX = frame?.x == null ? null : Number(frame.x);
  const rawY = frame?.y == null ? null : Number(frame.y);
  return {
    x: Number.isFinite(rawX) ? Math.round(rawX) : null,
    y: Number.isFinite(rawY) ? Math.round(rawY) : null,
    width: Math.round(width),
    height: Math.round(height),
  };
}

export function loadCompactWindowMemory(storage = browserStorage()) {
  const empty = { bar: null, browse: null, lastMode: null };
  if (!storage) return empty;
  try {
    const parsed = JSON.parse(storage.getItem(COMPACT_WINDOW_MEMORY_KEY));
    return {
      bar: normalizeCompactWindowFrame(parsed?.bar),
      browse: normalizeCompactWindowFrame(parsed?.browse),
      lastMode: ['bar', 'browse'].includes(parsed?.lastMode)
        ? parsed.lastMode
        : null,
    };
  } catch (_) {
    return empty;
  }
}

export function rememberCompactWindowFrame(frame, storage = browserStorage()) {
  const normalized = normalizeCompactWindowFrame(frame);
  const memory = loadCompactWindowMemory(storage);
  if (!normalized || !storage) return memory;
  const mode = isMiniWindowSize(normalized) ? 'bar' : 'browse';
  const next = { ...memory, [mode]: normalized, lastMode: mode };
  try {
    storage.setItem(COMPACT_WINDOW_MEMORY_KEY, JSON.stringify(next));
  } catch (_) {
    // 无痕模式或磁盘配额异常不应阻断窗口切换，本次会话仍可继续。
  }
  return next;
}

export function hasRememberedBarFrame(storage = browserStorage()) {
  return Boolean(loadCompactWindowMemory(storage).bar);
}

export function buildCompactWindowTransitionFrame(currentFrame, targetFrame) {
  const current = normalizeCompactWindowFrame(currentFrame);
  const target = normalizeCompactWindowFrame(targetFrame);
  if (!current || !target) return null;

  return {
    x: current.x,
    y: current.y,
    width: target.width,
    height: target.height,
  };
}

export function isCompactWindowPhysicalSize(size, scaleFactor = 1) {
  const scale = Number(scaleFactor);
  const safeScale = Number.isFinite(scale) && scale > 0 ? scale : 1;
  return isMiniWindowSize({
    width: Number(size?.width) / safeScale,
    height: Number(size?.height) / safeScale,
  });
}

async function captureCurrentCompactWindowSnapshot() {
  if (electronRenderer) {
    const snapshot = await electronRenderer.invoke('getCompactWindowSnapshot');
    return {
      frame: normalizeCompactWindowFrame(snapshot?.frame),
      normal: !snapshot?.maximized && !snapshot?.fullscreen,
    };
  }
  if (!isTauriRuntime) return null;

  const { getCurrentWindow } = await import('@tauri-apps/api/window');
  const window = getCurrentWindow();
  const [size, position, scaleFactor, maximized, fullscreen] = await Promise.all([
    window.innerSize(),
    window.outerPosition(),
    window.scaleFactor(),
    window.isMaximized(),
    window.isFullscreen(),
  ]);
  return {
    frame: normalizeCompactWindowFrame({
      x: position.x,
      y: position.y,
      // 尺寸用逻辑像素记忆，跨 Retina/普通屏幕后视觉大小才不会翻倍或减半。
      width: size.width / scaleFactor,
      height: size.height / scaleFactor,
    }),
    normal: !maximized && !fullscreen,
  };
}

async function applyTauriCompactWindowFrame(frame) {
  const { getCurrentWindow } = await import('@tauri-apps/api/window');
  const window = getCurrentWindow();
  if (await window.isFullscreen()) {
    await window.setFullscreen(false);
    // macOS 退出原生全屏有动画，动画未结束时紧接着 set_size 会被系统忽略。
    // 轮询状态后再留一帧稳定时间，普通窗口不走这条路径。
    const deadline = Date.now() + 1500;
    while ((await window.isFullscreen()) && Date.now() < deadline) {
      await new Promise(resolve => setTimeout(resolve, 50));
    }
    await new Promise(resolve => setTimeout(resolve, 350));
  }
  const { invoke } = await import('@tauri-apps/api/core');
  await invoke('restore_compact_window', frame);
  return true;
}

async function applyCompactWindowFrame(frame, electronChannel) {
  if (electronRenderer) {
    return electronRenderer.invoke(electronChannel, frame);
  }
  if (!isTauriRuntime) return false;
  return applyTauriCompactWindowFrame(frame);
}

export async function restoreRememberedCompactWindowFrame() {
  const memory = loadCompactWindowMemory();
  const mode =
    memory.lastMode || (memory.browse ? 'browse' : memory.bar ? 'bar' : null);
  const frame = mode ? memory[mode] : null;
  if (!frame) return null;
  const restored = await applyCompactWindowFrame(
    frame,
    'restoreRememberedCompactWindowFrame'
  );
  return restored ? { mode, frame } : null;
}

export async function rememberCurrentCompactWindowFrame() {
  const snapshot = await captureCurrentCompactWindowSnapshot();
  const frame = snapshot?.frame;
  // 最大化/全屏尺寸不是用户的普通窗口尺寸，不能覆盖 browse 记忆。
  if (!frame || !snapshot.normal) return null;
  rememberCompactWindowFrame(frame);
  return {
    mode: isMiniWindowSize(frame) ? 'bar' : 'browse',
    frame,
  };
}

export async function expandCompactWindow() {
  if (compactWindowTransitioning) return false;
  const snapshot = await captureCurrentCompactWindowSnapshot();
  const current = snapshot?.frame;
  // 某些 Windows/Linux 窗口管理器会先处理双击最大化。此时仍要让原生层
  // 退出最大化并恢复 browse，只是不记录这个临时的屏幕尺寸。
  if (!current || (snapshot.normal && !isMiniWindowSize(current))) return false;

  const memory = snapshot.normal
    ? rememberCompactWindowFrame(current)
    : loadCompactWindowMemory();
  const rememberedTarget =
    memory.browse ||
    normalizeCompactWindowFrame({ ...COMPACT_EXPANDED_SIZE, x: null, y: null });
  // 档位只负责恢复尺寸；位置跟随当前窗口，避免双屏时跳回另一块屏的旧坐标。
  const target = buildCompactWindowTransitionFrame(current, rememberedTarget);
  if (!target) return false;
  compactWindowTransitioning = true;
  try {
    const expanded = await applyCompactWindowFrame(
      target,
      'expandCompactWindow'
    );
    if (expanded) rememberCompactWindowFrame(target);
    return expanded;
  } finally {
    compactWindowTransitioning = false;
  }
}

export async function restoreCompactWindow() {
  if (compactWindowTransitioning) return false;
  const snapshot = await captureCurrentCompactWindowSnapshot();
  const current = snapshot?.frame;
  if (!current || (snapshot.normal && isMiniWindowSize(current))) return false;

  const memory = snapshot.normal
    ? rememberCompactWindowFrame(current)
    : loadCompactWindowMemory();
  if (!memory.bar) return false;
  // ESC 收回时留在用户正在操作的屏幕，不套用播放条上一次所在屏幕的绝对坐标。
  const target = buildCompactWindowTransitionFrame(current, memory.bar);
  if (!target) return false;
  compactWindowTransitioning = true;
  try {
    const restored = await applyCompactWindowFrame(
      target,
      'restoreCompactWindow'
    );
    if (restored) rememberCompactWindowFrame(target);
    return restored;
  } finally {
    compactWindowTransitioning = false;
  }
}
