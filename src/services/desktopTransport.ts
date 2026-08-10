import { isTauriRuntime } from '@/utils/runtime';

const tauriCommandNames = {
  isAlwaysOnTop: 'is_always_on_top',
  toggleAlwaysOnTop: 'toggle_always_on_top',
} as const;

export type DesktopCommand = keyof typeof tauriCommandNames;

export function sendDesktop(
  channel: string,
  payload: unknown = null
): Promise<unknown> {
  if (isTauriRuntime) {
    return import('@tauri-apps/api/core').then(({ invoke }) =>
      invoke('desktop_event', { channel, payload })
    );
  }
  return Promise.resolve();
}

export function invokeDesktop(channel: DesktopCommand): Promise<unknown> {
  if (isTauriRuntime) {
    return import('@tauri-apps/api/core').then(({ invoke }) =>
      invoke(tauriCommandNames[channel])
    );
  }
  return Promise.resolve(null);
}

export async function startDesktopWindowDragging(): Promise<void> {
  if (!isTauriRuntime) return;
  const { getCurrentWindow } = await import('@tauri-apps/api/window');
  await getCurrentWindow().startDragging();
}

export async function relaunchDesktop(): Promise<void> {
  if (!isTauriRuntime) return;
  const { relaunch } = await import('@tauri-apps/plugin-process');
  await relaunch();
}
