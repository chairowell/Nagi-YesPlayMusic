import { sendDesktop } from '@/services/desktopTransport';

type DesktopSender = (channel: string, payload: unknown) => Promise<unknown>;

export interface OrderedDesktopSettings {
  sync(settings: unknown): Promise<void>;
  sendDiscordPresence(payload: unknown): Promise<void>;
}

function jsonSnapshot(value: unknown): unknown {
  const serialized = JSON.stringify(value);
  if (serialized === undefined) {
    throw new TypeError('Desktop payload must be JSON serializable');
  }
  return JSON.parse(serialized) as unknown;
}

export function createOrderedDesktopSettings(
  send: DesktopSender
): OrderedDesktopSettings {
  let releaseFirstSettings: (() => void) | null = null;
  const firstSettings = new Promise<void>(resolve => {
    releaseFirstSettings = resolve;
  });
  let settingsQueue = Promise.resolve();

  return {
    sync(settings) {
      const snapshot = jsonSnapshot(settings);
      settingsQueue = settingsQueue
        .catch(() => undefined)
        .then(async () => {
          await send('settings', snapshot);
        });
      releaseFirstSettings?.();
      releaseFirstSettings = null;
      return settingsQueue;
    },
    async sendDiscordPresence(payload) {
      const snapshot = jsonSnapshot(payload);
      await firstSettings;
      await settingsQueue;
      await send('discordPresence', snapshot);
    },
  };
}

const desktopSettings = createOrderedDesktopSettings(sendDesktop);

export function syncDesktopSettings(settings: unknown): Promise<void> {
  return desktopSettings.sync(settings);
}

export function sendConfiguredDiscordPresence(payload: unknown): Promise<void> {
  return desktopSettings.sendDiscordPresence(payload);
}
