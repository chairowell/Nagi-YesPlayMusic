<template>
  <div
    class="win32-titlebar"
    data-tauri-drag-region
    @dblclick="windowMaxRestore"
  >
    <div class="title" data-tauri-drag-region>{{ title }}</div>
    <div class="controls">
      <div
        class="button minimize codicon codicon-chrome-minimize"
        @click="windowMinimize"
      ></div>
      <div
        class="button max-restore codicon"
        :class="{
          'codicon-chrome-restore': isMaximized,
          'codicon-chrome-maximize': !isMaximized,
        }"
        @click="windowMaxRestore"
      ></div>
      <div
        class="button close codicon codicon-chrome-close"
        @click="windowClose"
      ></div>
    </div>
  </div>
</template>

<script lang="ts">
import { defineComponent } from 'vue';
// icons by https://github.com/microsoft/vscode-codicons
import 'vscode-codicons/dist/codicon.css';

import { mapState } from 'pinia';
import { useAppStore } from '@/stores/app';
import { sendDesktop } from '@/services/desktopTransport';
import { isTauriRuntime } from '@/utils/runtime';
import type { UnlistenFn } from '@tauri-apps/api/event';

export default defineComponent({
  name: 'Win32Titlebar',
  data() {
    return {
      isMaximized: false,
      stopMaximizeListener: null as UnlistenFn | null,
      listenerDisposed: false,
    };
  },
  computed: {
    ...mapState(useAppStore, ['title']),
  },
  created() {
    if (isTauriRuntime) {
      import('@tauri-apps/api/event').then(async ({ listen }) => {
        const stop = await listen<boolean>('desktop://isMaximized', event => {
          this.isMaximized = event.payload;
        });
        if (this.listenerDisposed) stop();
        else this.stopMaximizeListener = stop;
      });
    }
  },
  beforeUnmount() {
    this.listenerDisposed = true;
    if (this.stopMaximizeListener) {
      this.stopMaximizeListener();
    }
  },
  methods: {
    windowMinimize() {
      void sendDesktop('minimize');
    },
    windowMaxRestore() {
      void sendDesktop('maximizeOrUnmaximize');
    },
    windowClose() {
      void sendDesktop('close');
    },
  },
});
</script>

<style lang="scss" scoped>
.win32-titlebar {
  color: var(--color-text);
  position: fixed;
  left: 0;
  top: 0;
  right: 0;
  display: flex;
  align-items: center;
  --hover: #e6e6e6;
  --active: #cccccc;

  .title {
    padding: 8px 12px;
    font-size: 12px;
    font-family: 'Segoe UI', 'Microsoft YaHei UI', 'Microsoft YaHei', sans-serif;
  }
  .controls {
    height: 32px;
    margin-left: auto;
    justify-content: flex-end;
    display: flex;
    .button {
      height: 100%;
      width: 46px;
      font-size: 16px;
      display: flex;
      justify-content: center;
      align-items: center;
      &:hover {
        background: var(--hover);
      }
      &:active {
        background: var(--active);
      }
      &.close {
        &:hover {
          background: #c42c1b;
          color: rgba(255, 255, 255, 0.8);
        }
        &:active {
          background: #f1707a;
          color: #000;
        }
      }
    }
  }
}
[data-theme='dark'] .win32-titlebar {
  --hover: #191919;
  --active: #333333;
}
</style>
