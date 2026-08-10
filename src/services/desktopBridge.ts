import { getAppStore } from '@/stores/accessor';
import { platform } from '@/utils/platform';
import { isDesktopRuntime } from '@/utils/runtime';
import { createDesktopEventHandlers } from '@/services/desktopEventHandlers';
import type { DesktopEventView } from '@/services/desktopEventHandlers';
import { syncDesktopSettings } from '@/services/desktopSettings';
import { createExternalLinkClickHandler } from '@/services/externalLinks';

export { sendDesktop, invokeDesktop } from '@/services/desktopTransport';

export async function connectDesktopEvents(
  self: DesktopEventView
): Promise<() => void> {
  if (!isDesktopRuntime) return () => {};

  const appStore = getAppStore();
  document.body.setAttribute('data-desktop', 'tauri');
  document.body.setAttribute('data-platform', platform);
  const externalLinkClick = createExternalLinkClickHandler();
  const handlers = createDesktopEventHandlers(self, appStore, appStore.player);

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
  // Register shortcuts only after native events have listeners.
  void syncDesktopSettings(appStore.settings);
  document.addEventListener('click', externalLinkClick);
  document.addEventListener('auxclick', externalLinkClick);
  return () => {
    document.removeEventListener('click', externalLinkClick);
    document.removeEventListener('auxclick', externalLinkClick);
    unlisten.forEach(stop => stop());
  };
}
