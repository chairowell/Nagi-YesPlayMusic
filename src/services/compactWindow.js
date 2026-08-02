import { electronRenderer } from '@/services/desktopTransport';
import { isTauriRuntime } from '@/utils/runtime';
import { isMiniWindowSize } from '@/utils/miniWindow';

export const COMPACT_EXPANDED_SIZE = Object.freeze({ width: 920, height: 620 });

let tauriMiniFrame = null;

export function isCompactWindowPhysicalSize(size, scaleFactor = 1) {
  const scale = Number(scaleFactor);
  const safeScale = Number.isFinite(scale) && scale > 0 ? scale : 1;
  return isMiniWindowSize({
    width: Number(size?.width) / safeScale,
    height: Number(size?.height) / safeScale,
  });
}

export async function expandCompactWindow() {
  if (electronRenderer) {
    return electronRenderer.invoke(
      'expandCompactWindow',
      COMPACT_EXPANDED_SIZE
    );
  }
  if (!isTauriRuntime || tauriMiniFrame) return false;

  const { getCurrentWindow, LogicalSize } = await import(
    '@tauri-apps/api/window'
  );
  const window = getCurrentWindow();
  const size = await window.innerSize();
  const scaleFactor = await window.scaleFactor();
  // Tauri 给的是物理像素；Retina 上不换算会把 494×254 的小窗误判为 988×508。
  if (!isCompactWindowPhysicalSize(size, scaleFactor)) return false;

  tauriMiniFrame = {
    size,
    position: await window.outerPosition(),
  };
  await window.setSize(
    new LogicalSize(COMPACT_EXPANDED_SIZE.width, COMPACT_EXPANDED_SIZE.height)
  );
  await window.center();
  return true;
}

export async function restoreCompactWindow() {
  if (electronRenderer) {
    return electronRenderer.invoke('restoreCompactWindow');
  }
  if (!isTauriRuntime || !tauriMiniFrame) return false;

  const frame = tauriMiniFrame;
  tauriMiniFrame = null;
  const { invoke } = await import('@tauri-apps/api/core');
  // 原生层掌握完整显示器工作区；在那里校验后再恢复，避免旧外接屏坐标让窗口消失。
  await invoke('restore_compact_window', {
    x: frame.position.x,
    y: frame.position.y,
    width: frame.size.width,
    height: frame.size.height,
  });
  return true;
}
