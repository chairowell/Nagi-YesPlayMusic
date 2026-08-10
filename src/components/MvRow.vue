<template>
  <div class="mv-row" :class="{ 'without-padding': withoutPadding }">
    <div v-for="mv in mvs" :key="getID(mv)" class="mv">
      <div
        class="cover"
        @mouseover="hoverVideoID = getID(mv)"
        @mouseleave="hoverVideoID = 0"
        @click="goToMv(getID(mv))"
      >
        <img :src="getUrl(mv)" loading="lazy" />
        <transition name="fade">
          <div
            v-show="hoverVideoID === getID(mv)"
            class="shadow"
            :style="{ background: 'url(' + getUrl(mv) + ')' }"
          ></div>
        </transition>
      </div>
      <div class="info">
        <div class="title">
          <router-link :to="'/mv/' + getID(mv)">{{ getTitle(mv) }}</router-link>
        </div>
        <div class="artist">
          <router-link
            v-if="subtitle === 'artist'"
            :to="`/artist/${getArtist(mv).id}`"
          >
            {{ getArtist(mv).name }}
          </router-link>
          <span v-else>{{ getSubtitle(mv) }}</span>
        </div>
      </div>
    </div>
  </div>
</template>

<script lang="ts">
import { defineComponent } from 'vue';
import type { PropType } from 'vue';
import { mapState } from 'pinia';
import { useAppStore } from '@/stores/app';
import type { MusicVideo } from '@/types/domain';
export default defineComponent({
  name: 'CoverVideo',
  props: {
    mvs: {
      type: Array as PropType<MusicVideo[]>,
      default: () => [],
    },
    subtitle: {
      type: String,
      default: 'artist',
    },
    withoutPadding: { type: Boolean, default: false },
  },
  data() {
    return {
      hoverVideoID: 0,
    };
  },
  computed: {
    ...mapState(useAppStore, ['player']),
  },
  methods: {
    goToMv(id: number): void {
      const query = { autoplay: String(this.player.playing) };
      this.$router.push({ path: '/mv/' + id, query });
    },
    getUrl(mv: MusicVideo): string {
      const url = mv.imgurl16v9 ?? mv.cover ?? mv.coverUrl ?? '';
      return url.replace(/^http:/, 'https:') + '?param=464y260';
    },
    getID(mv: MusicVideo): number {
      return mv.id ?? mv.vid ?? 0;
    },
    getTitle(mv: MusicVideo): string {
      return mv.name ?? mv.title ?? '';
    },
    getArtist(mv: MusicVideo): { name: string; id: number } {
      if (mv.artistName !== undefined) {
        return { name: mv.artistName, id: mv.artistId ?? 0 };
      }
      const creator = mv.creator?.[0];
      return creator
        ? { name: creator.userName ?? '', id: creator.userId ?? 0 }
        : { name: 'null', id: 0 };
    },
    getSubtitle(mv: MusicVideo): string {
      if (this.subtitle === 'publishTime') {
        return mv.publishTime ?? '';
      }
      return '';
    },
  },
});
</script>

<style lang="scss" scoped>
.mv-row {
  --col-num: 5;
  display: grid;
  grid-template-columns: repeat(var(--col-num), 1fr);
  gap: 36px 24px;
  padding: var(--main-content-padding);
}

.mv-row.without-padding {
  padding: 0;
}

@media (max-width: 900px) {
  .mv-row {
    --col-num: 4;
  }
}

@media (max-width: 800px) {
  .mv-row {
    --col-num: 3;
  }
}

@media (max-width: 700px) {
  .mv-row {
    --col-num: 2;
  }
}

@media (max-width: 550px) {
  .mv-row {
    --col-num: 1;
  }
}

.mv {
  color: var(--color-text);

  .title {
    font-size: 16px;
    font-weight: 600;
    opacity: 0.88;
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    overflow: hidden;
    word-break: break-all;
  }
  .artist {
    font-size: 12px;
    opacity: 0.68;
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    overflow: hidden;
  }
}

.cover {
  position: relative;
  transition: transform 0.3s;
  &:hover {
    cursor: pointer;
  }
}
img {
  border-radius: 0.75em;
  width: 100%;
  user-select: none;
}

.shadow {
  position: absolute;
  top: 6px;
  height: 100%;
  width: 100%;
  filter: blur(16px) opacity(0.4);
  transform: scale(0.9, 0.9);
  z-index: -1;
  background-size: cover;
  border-radius: 0.75em;
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
