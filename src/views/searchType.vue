<template>
  <div v-show="show" class="search">
    <h1>
      <span>{{ $t('search.searchFor') }} {{ typeNameTable[type] }}</span> "{{
        keywords
      }}"
    </h1>

    <div v-if="type === 'artists'">
      <CoverRow type="artist" :items="artistResults" :column-number="6" />
    </div>
    <div v-if="type === 'albums'">
      <CoverRow
        type="album"
        :items="albumResults"
        sub-text="artist"
        sub-text-font-size="14px"
      />
    </div>
    <div v-if="type === 'tracks'">
      <TrackList
        :tracks="trackResults"
        type="playlist"
        dbclick-track-func="playAList"
      />
    </div>
    <div v-if="type === 'musicVideos'">
      <MvRow :mvs="mvResults" />
    </div>
    <div v-if="type === 'playlists'">
      <CoverRow type="playlist" :items="playlistResults" sub-text="title" />
    </div>

    <div class="load-more">
      <ButtonTwoTone v-show="hasMore" color="grey" @click="fetchData">{{
        $t('explore.loadMore')
      }}</ButtonTwoTone>
    </div>
  </div>
</template>

<script lang="ts">
import { defineComponent } from 'vue';
import { getTrackDetail } from '@/api/track';
import { search } from '@/api/others';
import locale from '@/locale';
import { camelCase } from 'change-case';
import NProgress from 'nprogress';

import TrackList from '@/components/TrackList.vue';
import MvRow from '@/components/MvRow.vue';
import CoverRow from '@/components/CoverRow.vue';
import ButtonTwoTone from '@/components/ButtonTwoTone.vue';
import type {
  Album,
  Artist,
  MusicVideo,
  Playlist,
  Track,
} from '@/types/domain';

type SearchType = 'musicVideos' | 'tracks' | 'albums' | 'artists' | 'playlists';

const searchTypes = new Set<SearchType>([
  'musicVideos',
  'tracks',
  'albums',
  'artists',
  'playlists',
]);

function normalizeSearchType(value: unknown): SearchType {
  const normalized = camelCase(String(value ?? 'tracks'));
  return searchTypes.has(normalized as SearchType)
    ? (normalized as SearchType)
    : 'tracks';
}

export default defineComponent({
  name: 'Search',
  components: {
    TrackList,
    MvRow,
    CoverRow,
    ButtonTwoTone,
  },
  data() {
    return {
      show: false,
      hasMore: true,
      trackResults: [] as Track[],
      albumResults: [] as Album[],
      artistResults: [] as Artist[],
      playlistResults: [] as Playlist[],
      mvResults: [] as MusicVideo[],
    };
  },
  computed: {
    keywords() {
      return String(this.$route.params['keywords'] ?? '');
    },
    type() {
      return normalizeSearchType(this.$route.params['type']);
    },
    typeNameTable(): Record<SearchType, string> {
      return {
        musicVideos: locale.t('search.mv'),
        tracks: locale.t('search.song'),
        albums: locale.t('search.album'),
        artists: locale.t('search.artist'),
        playlists: locale.t('search.playlist'),
      };
    },
    resultLength(): number {
      const lengths: Record<SearchType, number> = {
        musicVideos: this.mvResults.length,
        tracks: this.trackResults.length,
        albums: this.albumResults.length,
        artists: this.artistResults.length,
        playlists: this.playlistResults.length,
      };
      return lengths[this.type];
    },
  },
  created() {
    this.fetchData();
  },
  methods: {
    fetchData() {
      const typeTable: Record<SearchType, number> = {
        musicVideos: 1004,
        tracks: 1,
        albums: 10,
        artists: 100,
        playlists: 1000,
      };
      return search({
        keywords: this.keywords,
        type: typeTable[this.type],
        offset: this.resultLength,
      }).then(response => {
        const result = response.result;
        if (!result) return;
        this.hasMore = result.hasMore ?? true;
        switch (this.type) {
          case 'musicVideos':
            this.mvResults.push(...(result.mvs ?? []));
            if (
              result.mvCount !== undefined &&
              result.mvCount <= this.mvResults.length
            ) {
              this.hasMore = false;
            }
            break;
          case 'artists':
            this.artistResults.push(...(result.artists ?? []));
            break;
          case 'albums':
            this.albumResults.push(...(result.albums ?? []));
            if (
              result.albumCount !== undefined &&
              result.albumCount <= this.albumResults.length
            ) {
              this.hasMore = false;
            }
            break;
          case 'tracks':
            this.trackResults.push(...(result.songs ?? []));
            this.getTracksDetail();
            break;
          case 'playlists':
            this.playlistResults.push(...(result.playlists ?? []));
            break;
        }
        NProgress.done();
        this.show = true;
      });
    },
    getTracksDetail() {
      const trackIDs = this.trackResults.map(track => track.id);
      if (trackIDs.length === 0) return;
      getTrackDetail(trackIDs.join(',')).then(result => {
        this.trackResults = result.songs;
      });
    },
  },
});
</script>

<style lang="scss" scoped>
h1 {
  margin-top: 32px;
  margin-bottom: 28px;
  color: var(--color-text);
  span {
    opacity: 0.58;
  }
}
.load-more {
  display: flex;
  justify-content: center;
  margin-top: 32px;
}

.button.more {
  .svg-icon {
    height: 24px;
    width: 24px;
  }
}
</style>
