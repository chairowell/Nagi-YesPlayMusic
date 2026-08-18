<template>
  <div ref="root" class="custom-select">
    <button
      type="button"
      class="custom-select-trigger"
      :class="{ open }"
      @click="toggle"
    >
      <span class="value">{{ currentLabel }}</span>
      <span class="arrow">
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M16.59 8.59 12 13.17 7.41 8.59 6 10l6 6 6-6z" />
        </svg>
      </span>
    </button>

    <Transition name="dropdown">
      <ul v-if="open" class="custom-select-menu">
        <li
          v-for="opt in options"
          :key="String(opt.value)"
          :class="{ active: opt.value === modelValue }"
          @click="select(opt.value)"
        >
          {{ opt.label }}
        </li>
      </ul>
    </Transition>
  </div>
</template>

<script lang="ts">
import { defineComponent, type PropType } from 'vue';

export interface SelectOption {
  value: string | number | boolean;
  label: string;
}

/**
 * 自定义下拉选择框，替代原生 <select>。
 */
export default defineComponent({
  name: 'CustomSelect',
  props: {
    modelValue: {
      type: [String, Number, Boolean] as PropType<string | number | boolean>,
      required: true,
    },
    options: {
      type: Array as PropType<SelectOption[]>,
      required: true,
    },
  },
  emits: ['update:modelValue'],
  data() {
    return {
      open: false,
    };
  },
  computed: {
    currentLabel(): string {
      const matched = this.options.find(opt => opt.value === this.modelValue);
      return matched?.label ?? '';
    },
  },
  methods: {
    toggle() {
      this.open = !this.open;
    },
    select(value: string | number | boolean) {
      this.$emit('update:modelValue', value);
      this.open = false;
    },
    onDocumentClick(event: MouseEvent) {
      const root = this.$refs['root'] as HTMLElement | undefined;
      if (root && !root.contains(event.target as Node)) {
        this.open = false;
      }
    },
  },
  mounted() {
    document.addEventListener('click', this.onDocumentClick);
  },
  unmounted() {
    document.removeEventListener('click', this.onDocumentClick);
  },
});
</script>

<style scoped lang="scss">
.custom-select {
  position: relative;
  display: inline-block;

  .custom-select-trigger {
    font-weight: 600;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    min-width: 192px;
    max-width: 600px;
    padding: 8px 12px;
    background: var(--color-secondary-bg);
    color: var(--color-text);
    border: none;
    border-radius: 8px;
    cursor: pointer;
    transition: background 0.2s ease, color 0.2s ease;
    user-select: none;

    .value {
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }

    .arrow {
      display: flex;
      align-items: center;
      transition: transform 0.2s ease;
      opacity: 0.6;

      svg {
        width: 20px;
        height: 20px;
        fill: currentColor;
      }
    }

    &:hover {
      background: var(--color-secondary-bg);
      color: var(--color-primary);
    }

    &.open {
      color: var(--color-primary);
      background: var(--color-primary-bg);

      .arrow {
        transform: rotate(180deg);
      }
    }
  }

  .custom-select-menu {
    font-weight: 600;
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    width: 100%;
    box-sizing: border-box;
    list-style: none;
    margin: 0;
    padding: 4px;
    background: var(--color-body-bg);
    border: 1px solid rgba(128, 128, 128, 0.18);
    border-radius: 8px;
    // 深色模式下勾勒菜单边缘，浅色模式下几乎不可见
    box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.1),
      0 6px 12px -4px rgba(0, 0, 0, 0.12);
    z-index: 100;
    overflow: hidden;

    li {
      padding: 8px 12px;
      border-radius: 6px;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
      font-size: 14px;
      cursor: pointer;
      color: var(--color-text);
      transition: background 0.15s ease, color 0.15s ease;

      &:hover {
        background: rgba(128, 128, 128, 0.15);
      }

      &.active {
        color: var(--color-primary);
        font-weight: 600;
      }
    }
  }
}

// 展开：先加速后撞击减速并轻微回弹
.dropdown-enter-active {
  transition: opacity 0.2s ease, transform 0.3s cubic-bezier(0.34, 1.2, 0.64, 1);
  transform-origin: top;
}

// 收起：快速利落收回
.dropdown-leave-active {
  transition: opacity 0.15s ease, transform 0.15s ease;
  transform-origin: top;
}

.dropdown-enter-from,
.dropdown-leave-to {
  opacity: 0;
  transform: translateY(-4px) scale(0.98);
}
</style>
