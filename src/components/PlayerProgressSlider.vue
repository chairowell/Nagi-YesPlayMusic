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

<script lang="ts">
import { defineComponent } from 'vue';
import VueSlider from 'vue-slider-component';
import {
  PLAYBACK_SLIDER_INTERVAL,
  normalizePlaybackSliderMax,
} from '@/utils/progressSliderScale';

interface SliderInstance {
  setValue(value: number): void;
  dragEnd(event: Event): void;
}

export default defineComponent({
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
  emits: {
    'update:modelValue': (value: number) => Number.isFinite(value),
  },
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
    commitValue(value: number) {
      if (!this.acceptCommits) return;
      this.$emit('update:modelValue', value);
    },
    finishDrag() {
      this.$nextTick(() => {
        const slider = this.$refs['slider'] as SliderInstance | undefined;
        if (!slider) return;
        // Adopt the player's resolved seek position after dragging.
        slider.setValue(this.modelValue);
      });
    },
    finishCancelledDrag(event: Event) {
      const slider = this.$refs['slider'] as SliderInstance | undefined;
      if (!slider) return;
      // Finish lazy drags cancelled by WKWebView.
      slider.dragEnd(event);
    },
  },
});
</script>
