<template>
  <vue-slider
    ref="slider"
    :model-value="modelValue"
    :lazy="true"
    v-bind="$attrs"
    @update:model-value="$emit('update:modelValue', $event)"
    @pointercancel.capture="finishCancelledDrag"
    @touchcancel.capture="finishCancelledDrag"
  ></vue-slider>
</template>

<script>
import VueSlider from 'vue-slider-component';

export default {
  name: 'PlayerProgressSlider',
  inheritAttrs: false,
  components: { VueSlider },
  props: {
    modelValue: {
      type: Number,
      required: true,
    },
  },
  emits: ['update:modelValue'],
  mounted() {
    window.addEventListener('blur', this.finishCancelledDrag);
  },
  beforeUnmount() {
    window.removeEventListener('blur', this.finishCancelledDrag);
  },
  methods: {
    finishCancelledDrag(event) {
      const slider = this.$refs.slider;
      if (!slider) return;
      // vue-slider 的 lazy 模式只监听 mouseup/touchend；WKWebView 取消手势时
      // 必须主动走 dragEnd，否则视觉预览会留在新位置，播放仍停在旧时间。
      slider.dragEnd(event);
    },
  },
};
</script>
