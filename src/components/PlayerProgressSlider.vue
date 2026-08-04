<template>
  <vue-slider
    ref="slider"
    v-bind="$attrs"
    :model-value="modelValue"
    :max="sliderMax"
    :interval="progressSliderInterval"
    :lazy="true"
    @update:model-value="commitValue"
    @drag-end="finishDrag"
    @pointercancel.capture="finishCancelledDrag"
    @touchcancel.capture="finishCancelledDrag"
  ></vue-slider>
</template>

<script>
import VueSlider from 'vue-slider-component';
import {
  PLAYBACK_SLIDER_INTERVAL,
  normalizePlaybackSliderMax,
} from '@/utils/progressSliderScale';

export default {
  name: 'PlayerProgressSlider',
  inheritAttrs: false,
  components: { VueSlider },
  props: {
    modelValue: {
      type: Number,
      required: true,
    },
    max: {
      type: Number,
      required: true,
    },
  },
  emits: ['update:modelValue'],
  data() {
    return {
      acceptCommits: true,
      progressSliderInterval: PLAYBACK_SLIDER_INTERVAL,
    };
  },
  computed: {
    sliderMax() {
      return normalizePlaybackSliderMax(this.max);
    },
  },
  mounted() {
    window.addEventListener('blur', this.finishCancelledDrag);
  },
  beforeUnmount() {
    this.acceptCommits = false;
    window.removeEventListener('blur', this.finishCancelledDrag);
  },
  methods: {
    commitValue(value) {
      if (!this.acceptCommits) return;
      this.$emit('update:modelValue', value);
    },
    finishDrag() {
      this.$nextTick(() => {
        const slider = this.$refs.slider;
        if (!slider) return;
        // lazy 滑块在 Drag 状态会忽略父级修正值；拖拽结束后主动采用
        // Player 在原生 seeked 后确认的位置，避免滑块与歌词各走各的。
        slider.setValue(this.modelValue);
      });
    },
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
