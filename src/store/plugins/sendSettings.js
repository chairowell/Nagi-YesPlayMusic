import { sendDesktop } from '@/services/desktopTransport';

export function getSendSettingsPlugin() {
  return store => {
    store.subscribe((mutation, state) => {
      // console.log(mutation);
      if (mutation.type !== 'updateSettings') return;
      void sendDesktop('settings', state.settings);
    });
  };
}
