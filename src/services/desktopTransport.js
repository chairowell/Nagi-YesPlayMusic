import { isElectronRuntime, isTauriRuntime } from '@/utils/runtime';

export const electronRenderer = isElectronRuntime
  ? window.require('electron').ipcRenderer
  : null;

const tauriCommandNames = {
  isAlwaysOnTop: 'is_always_on_top',
  toggleAlwaysOnTop: 'toggle_always_on_top',
};

export function sendDesktop(channel, payload = null) {
  if (electronRenderer) {
    electronRenderer.send(channel, payload);
    return Promise.resolve();
  }
  if (isTauriRuntime) {
    return import('@tauri-apps/api/core').then(({ invoke }) =>
      invoke('desktop_event', { channel, payload })
    );
  }
  return Promise.resolve();
}

export function invokeDesktop(channel, ...args) {
  if (electronRenderer) return electronRenderer.invoke(channel, ...args);
  if (isTauriRuntime && tauriCommandNames[channel]) {
    return import('@tauri-apps/api/core').then(({ invoke }) =>
      invoke(tauriCommandNames[channel])
    );
  }
  return Promise.resolve(null);
}
