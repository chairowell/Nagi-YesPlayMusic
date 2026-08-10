/// <reference types="vite/client" />

import type Player from '@/utils/Player';
import type filters from '@/utils/filters';
import type { copyText } from '@/utils/clipboard';
import type { AppShell } from '@/types/appShell';

declare module 'vue' {
  interface ComponentCustomProperties {
    $filters: typeof filters;
    $copyText: typeof copyText;
    appShell: AppShell;
  }
}

declare global {
  interface Window {
    resetApp: () => string;
    yesplaymusic?: {
      player?: Player;
    };
    webkitOfflineAudioContext: typeof OfflineAudioContext;
  }

  interface Navigator {
    userAgentData?: {
      platform?: string;
    };
  }
}

export {};
