import store from '@/store';
import { platform } from '@/utils/platform';
import {
  isDesktopRuntime,
} from '@/utils/runtime';
import { createDesktopEventHandlers } from '@/services/desktopEventHandlers';
import { electronRenderer } from '@/services/desktopTransport';

export { sendDesktop, invokeDesktop } from '@/services/desktopTransport';

export async function connectDesktopEvents(self) {
  if (!isDesktopRuntime) return () => {};

  document.body.setAttribute('data-electron', 'yes');
  document.body.setAttribute('data-electron-os', platform);
  const handlers = createDesktopEventHandlers(self, store, store.state.player);

  if (electronRenderer) {
    const listeners = Object.entries(handlers).map(([channel, handler]) => {
      const listener = (_, payload) => handler(payload);
      electronRenderer.on(channel, listener);
      return [channel, listener];
    });
    return () => {
      for (const [channel, listener] of listeners) {
        electronRenderer.removeListener(channel, listener);
      }
    };
  }

  const { listen } = await import('@tauri-apps/api/event');
  const unlisten = await Promise.all(
    Object.entries(handlers).map(([channel, handler]) =>
      listen(`desktop://${channel}`, event => handler(event.payload))
    )
  );
  return () => unlisten.forEach(stop => stop());
}
