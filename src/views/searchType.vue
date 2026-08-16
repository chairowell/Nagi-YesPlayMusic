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
import {
  searchAlbums,
  searchArtists,
  searchMusicVideos,
  searchPlaylists,
  searchTracks,
} from '@/services/searchSource';
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
      const keywords = this.keywords;
      const options = { offset: this.resultLength };
      // Every channel now pages against the server-reported total, which
      // the legacy hasMore flag only approximated for two of them.
      const finish = (accumulated: number, total: number) => {
        this.hasMore = accumulated < total;
        NProgress.done();
        this.show = true;
      };
      switch (this.type) {
        case 'musicVideos':
          return searchMusicVideos(keywords, options).then(page => {
            this.mvResults.push(...page.items);
            finish(this.mvResults.length, page.total);
          });
        case 'artists':
          return searchArtists(keywords, options).then(page => {
            this.artistResults.push(...page.items);
            finish(this.artistResults.length, page.total);
          });
        case 'albums':
          return searchAlbums(keywords, options).then(page => {
            this.albumResults.push(...page.items);
            finish(this.albumResults.length, page.total);
          });
        case 'tracks':
          return searchTracks(keywords, options).then(page => {
            this.trackResults.push(...page.items);
            this.getTracksDetail();
            finish(this.trackResults.length, page.total);
          });
        case 'playlists':
          return searchPlaylists(keywords, options).then(page => {
            this.playlistResults.push(...page.items);
            finish(this.playlistResults.length, page.total);
          });
      }
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
