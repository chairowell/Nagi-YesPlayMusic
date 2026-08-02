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

export function isCompactWindowPhysicalSize(size, scaleFactor = 1) {
  const scale = Number(scaleFactor);
  const safeScale = Number.isFinite(scale) && scale > 0 ? scale : 1;
  return isMiniWindowSize({
    width: Number(size?.width) / safeScale,
    height: Number(size?.height) / safeScale,
  });
}

async function captureCurrentCompactWindowFrame() {
  if (electronRenderer) {
    return normalizeCompactWindowFrame(
      await electronRenderer.invoke('getCompactWindowFrame')
    );
  }
  if (!isTauriRuntime) return null;

  const { getCurrentWindow } = await import('@tauri-apps/api/window');
  const window = getCurrentWindow();
  const [size, position, scaleFactor] = await Promise.all([
    window.innerSize(),
    window.outerPosition(),
    window.scaleFactor(),
  ]);
  return normalizeCompactWindowFrame({
    x: position.x,
    y: position.y,
    // 尺寸用逻辑像素记忆，跨 Retina/普通屏幕后视觉大小才不会翻倍或减半。
    width: size.width / scaleFactor,
    height: size.height / scaleFactor,
  });
}

async function applyTauriCompactWindowFrame(frame) {
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
  const frame = await captureCurrentCompactWindowFrame();
  if (!frame) return null;
  rememberCompactWindowFrame(frame);
  return {
    mode: isMiniWindowSize(frame) ? 'bar' : 'browse',
    frame,
  };
}

export async function expandCompactWindow() {
  if (compactWindowTransitioning) return false;
  const current = await captureCurrentCompactWindowFrame();
  if (!current || !isMiniWindowSize(current)) return false;

  const memory = rememberCompactWindowFrame(current);
  const target =
    memory.browse ||
    normalizeCompactWindowFrame({ ...COMPACT_EXPANDED_SIZE, x: null, y: null });
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
  const current = await captureCurrentCompactWindowFrame();
  if (!current || isMiniWindowSize(current)) return false;

  const memory = rememberCompactWindowFrame(current);
  if (!memory.bar) return false;
  compactWindowTransitioning = true;
  try {
    const restored = await applyCompactWindowFrame(
      memory.bar,
      'restoreCompactWindow'
    );
    if (restored) rememberCompactWindowFrame(memory.bar);
    return restored;
  } finally {
    compactWindowTransitioning = false;
  }
}
