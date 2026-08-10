<template>
  <div class="next-tracks">
    <h1>{{ $t('next.nowPlaying') }}</h1>
    <TrackList
      :tracks="[currentTrack]"
      type="playlist"
      dbclick-track-func="none"
    />
    <h1 v-show="playNextList.length > 0"
      >插队播放
      <button @click="player.clearPlayNextList()">清除队列</button>
    </h1>
    <TrackList
      v-show="playNextList.length > 0"
      :tracks="playNextTracks"
      type="playlist"
      :highlight-playing-track="false"
      dbclick-track-func="playQueuedTrackByID"
      item-key="id+index"
      :extra-context-menu-item="['removeTrackFromQueue']"
    />
    <h1>{{ $t('next.nextUp') }}</h1>
    <TrackList
      :tracks="filteredTracks"
      type="playlist"
      :highlight-playing-track="false"
      dbclick-track-func="playTrackOnListByID"
    />
  </div>
</template>

<script lang="ts">
import { defineComponent } from 'vue';
import { mapState } from 'pinia';
import { useAppStore } from '@/stores/app';
import { getTrackDetail } from '@/api/track';
import TrackList from '@/components/TrackList.vue';
import type { Track } from '@/types/domain';

export default defineComponent({
  name: 'Next',
  inject: ['appShell'],
  components: {
    TrackList,
  },
  data() {
    return {
      tracks: [] as Track[],
    };
  },
  computed: {
    ...mapState(useAppStore, ['player']),
    currentTrack() {
      return this.player.currentTrack;
    },
    playerShuffle() {
      return this.player.shuffle;
    },
    filteredTracks(): Track[] {
      const trackIDs = this.player.list.slice(
        this.player.current + 1,
        this.player.current + 100
      );
      return trackIDs
        .map(tid => this.tracks.find(t => t.id === tid))
        .filter((track): track is Track => track !== undefined);
    },
    playNextList() {
      return this.player.playNextList;
    },
    playNextTracks(): Track[] {
      return this.playNextList
        .map(trackId => this.tracks.find(track => track.id === trackId))
        .filter((track): track is Track => track !== undefined);
    },
  },
  watch: {
    currentTrack() {
      this.loadTracks();
    },
    playerShuffle() {
      this.loadTracks();
    },
    playNextList() {
      this.loadTracks();
    },
  },
  activated() {
    this.loadTracks();
    this.appShell.restoreScrollPosition();
  },
  methods: {
    loadTracks() {
      // Queue the next 100 tracks.
      const trackIDs = this.player.list.slice(
        this.player.current + 1,
        this.player.current + 100
      );

      // Place priority tracks ahead of the regular queue.
      trackIDs.push(...this.playNextList);

      // Skip tracks already loaded.
      const loadedTrackIDs = this.tracks.map(t => t.id);

      if (trackIDs.length > 0) {
        getTrackDetail(trackIDs.join(',')).then(data => {
          const newTracks = data.songs.filter(
            t => !loadedTrackIDs.includes(t.id)
          );
          this.tracks.push(...newTracks);
        });
      }
    },
  },
});
</script>

<style lang="scss" scoped>
h1 {
  margin-top: 36px;
  margin-bottom: 18px;
  cursor: default;
  color: var(--color-text);
  display: flex;
  justify-content: space-between;
  button {
    color: var(--color-text);
    border-radius: 8px;
    padding: 0 14px;
    display: flex;
    justify-content: center;
    align-items: center;
    transition: 0.2s;
    opacity: 0.68;
    font-weight: 500;
    &:hover {
      opacity: 1;
      background: var(--color-secondary-bg);
    }
    &:active {
      opacity: 1;
      transform: scale(0.92);
    }
  }
}
</style>
