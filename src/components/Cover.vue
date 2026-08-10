<template>
  <div
    class="cover"
    :class="{ 'cover-hover': coverHover }"
    @mouseover="focus = true"
    @mouseleave="focus = false"
    @click="clickCoverToPlay ? play() : goTo()"
  >
    <div class="cover-container">
      <div class="shade">
        <button
          v-show="focus"
          class="play-button"
          :style="playButtonStyles"
          @click.stop="play()"
          ><svg-icon icon-class="play" />
        </button>
      </div>
      <img :src="imageUrl" :style="imageStyles" loading="lazy" />
      <transition v-if="coverHover || alwaysShowShadow" name="fade">
        <div
          v-show="focus || alwaysShowShadow"
          class="shadow"
          :style="shadowStyles"
        ></div>
      </transition>
    </div>
  </div>
</template>

<script lang="ts">
import { defineComponent } from 'vue';
import type { CSSProperties, PropType } from 'vue';
import { mapState } from 'pinia';
import { useAppStore } from '@/stores/app';
export default defineComponent({
  props: {
    id: { type: Number, required: true },
    type: {
      type: String as PropType<'album' | 'playlist' | 'artist'>,
      required: true,
    },
    imageUrl: { type: String, required: true },
    fixedSize: { type: Number, default: 0 },
    playButtonSize: { type: Number, default: 22 },
    coverHover: { type: Boolean, default: true },
    alwaysShowPlayButton: { type: Boolean, default: true },
    alwaysShowShadow: { type: Boolean, default: false },
    clickCoverToPlay: { type: Boolean, default: false },
    shadowMargin: { type: Number, default: 12 },
    radius: { type: Number, default: 12 },
  },
  data() {
    return {
      focus: false,
    };
  },
  computed: {
    ...mapState(useAppStore, ['player']),
    imageStyles(): CSSProperties {
      const styles: CSSProperties = {};
      if (this.fixedSize !== 0) {
        styles.width = this.fixedSize + 'px';
        styles.height = this.fixedSize + 'px';
      }
      if (this.type === 'artist') styles.borderRadius = '50%';
      return styles;
    },
    playButtonStyles(): CSSProperties {
      const styles: CSSProperties = {};
      styles.width = this.playButtonSize + '%';
      styles.height = this.playButtonSize + '%';
      return styles;
    },
    shadowStyles(): CSSProperties {
      const styles: CSSProperties = {};
      styles.backgroundImage = `url(${this.imageUrl})`;
      if (this.type === 'artist') styles.borderRadius = '50%';
      return styles;
    },
  },
  methods: {
    play() {
      const player = this.player;
      const playActions: Record<
        'album' | 'playlist' | 'artist',
        (id: number) => void
      > = {
        album: id => player.playAlbumByID(id),
        playlist: id => player.playPlaylistByID(id),
        artist: id => player.playArtistByID(id),
      };
      playActions[this.type](this.id);
    },
    goTo() {
      this.$router.push({ name: this.type, params: { id: this.id } });
    },
  },
});
</script>

<style lang="scss" scoped>
.cover {
  position: relative;
  transition: transform 0.3s;
}
.cover-container {
  position: relative;
}
img {
  border-radius: 0.75em;
  width: 100%;
  user-select: none;
  aspect-ratio: 1 / 1;
  border: 1px solid rgba(0, 0, 0, 0.04);
}

.cover-hover {
  &:hover {
    cursor: pointer;
    /* transform: scale(1.02); */
  }
}

.shade {
  position: absolute;
  top: 0;
  height: calc(100% - 3px);
  width: 100%;
  background: transparent;
  display: flex;
  justify-content: center;
  align-items: center;
}
.play-button {
  display: flex;
  justify-content: center;
  align-items: center;
  color: white;
  backdrop-filter: blur(8px);
  background: rgba(255, 255, 255, 0.14);
  border: 1px solid rgba(255, 255, 255, 0.08);
  height: 22%;
  width: 22%;
  border-radius: 50%;
  cursor: default;
  transition: 0.2s;
  .svg-icon {
    width: 50%;
    margin: {
      left: 4px;
    }
  }
  &:hover {
    background: rgba(255, 255, 255, 0.28);
  }
  &:active {
    transform: scale(0.94);
  }
}

.shadow {
  position: absolute;
  top: 12px;
  height: 100%;
  width: 100%;
  filter: blur(16px) opacity(0.6);
  transform: scale(0.92, 0.96);
  z-index: -1;
  background-size: cover;
  border-radius: 0.75em;
  aspect-ratio: 1 / 1;
}

.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.3s;
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
