<template>
  <div ref="contextMenu" class="context-menu">
    <div
      v-if="showMenu"
      ref="menu"
      class="menu"
      tabindex="-1"
      :style="{ top: top, left: left }"
      @blur="closeMenu"
      @click="closeMenu"
    >
      <slot></slot>
    </div>
  </div>
</template>

<script lang="ts">
import { defineComponent } from 'vue';
import { mapActions, mapState } from 'pinia';
import { useAppStore } from '@/stores/app';

export default defineComponent({
  name: 'ContextMenu',
  emits: {
    close: () => true,
  },
  data() {
    return {
      showMenu: false,
      top: '0px',
      left: '0px',
    };
  },
  computed: {
    ...mapState(useAppStore, ['player']),
  },
  methods: {
    ...mapActions(useAppStore, ['enableScrollingWith']),
    setMenu(top: number, left: number) {
      let heightOffset = this.player.enabled ? 64 : 0;
      const menu = this.$refs['menu'] as HTMLDivElement;
      let largestHeight = window.innerHeight - menu.offsetHeight - heightOffset;
      let largestWidth = window.innerWidth - menu.offsetWidth - 25;
      if (top > largestHeight) top = largestHeight;
      if (left > largestWidth) left = largestWidth;
      this.top = top + 'px';
      this.left = left + 'px';
    },

    closeMenu() {
      this.showMenu = false;
      this.$emit('close');
      this.enableScrollingWith(true);
    },

    openMenu(e: MouseEvent) {
      this.showMenu = true;
      this.$nextTick(() => {
        (this.$refs['menu'] as HTMLDivElement).focus();
        this.setMenu(e.y, e.x);
      });
      e.preventDefault();
      this.enableScrollingWith(false);
    },
  },
});
</script>

<style lang="scss" scoped>
.context-menu {
  width: 100%;
  height: 100%;
  user-select: none;
}

.menu {
  position: fixed;
  min-width: 136px;
  max-width: 240px;
  list-style: none;
  background: rgba(255, 255, 255, 0.88);
  box-shadow: 0 6px 12px -4px rgba(0, 0, 0, 0.08);
  border: 1px solid rgba(0, 0, 0, 0.06);
  backdrop-filter: blur(12px);
  border-radius: 12px;
  box-sizing: border-box;
  padding: 6px;
  z-index: 1000;
  -webkit-app-region: no-drag;
  transition: background 125ms ease-out, opacity 125ms ease-out,
    transform 125ms ease-out;

  &:focus {
    outline: none;
  }
}

[data-theme='dark'] {
  .menu {
    background: rgba(36, 36, 36, 0.78);
    backdrop-filter: blur(16px) contrast(120%) brightness(60%);
    border: 1px solid rgba(255, 255, 255, 0.08);
    box-shadow: 0 0 6px rgba(255, 255, 255, 0.08);
  }
  .menu :deep(.item:hover) {
    color: var(--color-text);
  }
}

@supports (-moz-appearance: none) {
  .menu {
    background-color: var(--color-body-bg) !important;
  }
}

.menu :deep(.item) {
  font-weight: 600;
  font-size: 14px;
  padding: 10px 14px;
  border-radius: 8px;
  cursor: default;
  color: var(--color-text);
  display: flex;
  align-items: center;
  &:hover {
    color: var(--color-primary);
    background: var(--color-primary-bg-for-transparent);
    transition: opacity 125ms ease-out, transform 125ms ease-out;
  }
  &:active {
    opacity: 0.75;
    transform: scale(0.95);
  }
}

.menu :deep(.item .svg-icon) {
  height: 16px;
  width: 16px;
  margin-right: 5px;
}

.menu :deep(hr) {
  margin: 4px 10px;
  background: rgba(128, 128, 128, 0.18);
  height: 1px;
  box-shadow: none;
  border: none;
}

.menu :deep(.item-info) {
  padding: 10px 10px;
  display: flex;
  align-items: center;
  color: var(--color-text);
  cursor: default;
}

.menu :deep(.item-info img) {
  height: 38px;
  width: 38px;
  border-radius: 4px;
}

.menu :deep(.item-info .info) {
  margin-left: 10px;
}

.menu :deep(.item-info .title) {
  font-size: 16px;
  font-weight: 600;
  display: -webkit-box;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 1;
  overflow: hidden;
  word-break: break-all;
}

.menu :deep(.item-info .subtitle) {
  font-size: 12px;
  opacity: 0.68;
  display: -webkit-box;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 1;
  overflow: hidden;
  word-break: break-all;
}
</style>
