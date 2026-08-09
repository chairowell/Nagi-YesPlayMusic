import store from '@/store';
import { platform } from '@/utils/platform';
import {
  isDesktopRuntime,
} from '@/utils/runtime';
import { createDesktopEventHandlers } from '@/services/desktopEventHandlers';
import {
  electronRenderer,
  sendDesktop,
} from '@/services/desktopTransport';
import { createExternalLinkClickHandler } from '@/services/externalLinks';

export { sendDesktop, invokeDesktop } from '@/services/desktopTransport';

export async function connectDesktopEvents(self) {
  if (!isDesktopRuntime) return () => {};

  document.body.setAttribute('data-electron', 'yes');
  document.body.setAttribute('data-electron-os', platform);
  const externalLinkClick = createExternalLinkClickHandler();
  const handlers = createDesktopEventHandlers(self, store, store.state.player);
  // Tauri 没有 Electron 主进程的持久化 store，启动时必须主动同步一次，
  // 否则全局快捷键要等用户改过任意设置后才会真正注册。
  void sendDesktop('settings', store.state.settings);

  if (electronRenderer) {
    const listeners = Object.entries(handlers).map(([channel, handler]) => {
      const listener = (_, payload) => handler(payload);
      electronRenderer.on(channel, listener);
      return [channel, listener];
    });
    document.addEventListener('click', externalLinkClick);
    return () => {
      document.removeEventListener('click', externalLinkClick);
      for (const [channel, listener] of listeners) {
        electronRenderer.removeListener(channel, listener);
      }
    };
  }

  const { listen } = await import('@tauri-apps/api/event');
  const subscriptions = await Promise.allSettled(
    Object.entries(handlers).map(([channel, handler]) =>
      listen(`desktop://${channel}`, event => handler(event.payload))
    )
  );
  const unlisten = subscriptions
    .filter(result => result.status === 'fulfilled')
    .map(result => result.value);
  const failed = subscriptions.find(result => result.status === 'rejected');
  if (failed) {
    unlisten.forEach(stop => stop());
    throw failed.reason;
  }
  document.addEventListener('click', externalLinkClick);
  return () => {
    document.removeEventListener('click', externalLinkClick);
    unlisten.forEach(stop => stop());
  };
}
