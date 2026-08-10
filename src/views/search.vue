<template>
  <div v-show="show" class="search-page">
    <div v-show="artists.length > 0 || albums.length > 0" class="row">
      <div v-show="artists.length > 0" class="artists">
        <div v-show="artists.length > 0" class="section-title"
          >{{ $t('search.artist')
          }}<router-link :to="`/search/${keywords}/artists`">{{
            $t('home.seeMore')
          }}</router-link></div
        >
        <CoverRow
          type="artist"
          :column-number="3"
          :items="artists.slice(0, 3)"
          gap="34px 24px"
        />
      </div>

      <div class="albums">
        <div v-show="albums.length > 0" class="section-title"
          >{{ $t('search.album')
          }}<router-link :to="`/search/${keywords}/albums`">{{
            $t('home.seeMore')
          }}</router-link></div
        >
        <CoverRow
          type="album"
          :items="albums.slice(0, 3)"
          sub-text="artist"
          :column-number="3"
          sub-text-font-size="14px"
          gap="34px 24px"
          :play-button-size="26"
        />
      </div>
    </div>

    <div v-show="tracks.length > 0" class="tracks">
      <div class="section-title"
        >{{ $t('search.song')
        }}<router-link :to="`/search/${keywords}/tracks`">{{
          $t('home.seeMore')
        }}</router-link></div
      >
      <TrackList :tracks="tracks" type="tracklist" />
    </div>

    <div v-show="musicVideos.length > 0" class="music-videos">
      <div class="section-title"
        >{{ $t('search.mv')
        }}<router-link :to="`/search/${keywords}/music-videos`">{{
          $t('home.seeMore')
        }}</router-link></div
      >
      <MvRow :mvs="musicVideos.slice(0, 5)" />
    </div>

    <div v-show="playlists.length > 0" class="playlists">
      <div class="section-title"
        >{{ $t('search.playlist')
        }}<router-link :to="`/search/${keywords}/playlists`">{{
          $t('home.seeMore')
        }}</router-link></div
      >
      <CoverRow
        type="playlist"
        :items="playlists.slice(0, 12)"
        sub-text="title"
        :column-number="6"
        sub-text-font-size="14px"
        gap="34px 24px"
        :play-button-size="26"
      />
    </div>

    <div v-show="!haveResult" class="no-results">
      <div
        ><svg-icon icon-class="search" />
        {{
          keywords.length === 0 ? '输入关键字搜索' : $t('search.noResult')
        }}</div
      >
    </div>
  </div>
</template>

<script lang="ts">
import { defineComponent } from 'vue';
import { mapActions, mapState } from 'pinia';
import { useAppStore } from '@/stores/app';
import { getTrackDetail } from '@/api/track';
import { search as searchApi } from '@/api/others';
import type { SearchResult } from '@/api/others';
import { settleIndependentRequests } from '@/services/searchBatches';
import NProgress from 'nprogress';

import TrackList from '@/components/TrackList.vue';
import MvRow from '@/components/MvRow.vue';
import CoverRow from '@/components/CoverRow.vue';
import type {
  Album,
  Artist,
  MusicVideo,
  Playlist,
  Track,
} from '@/types/domain';

type SearchKind = 'musicVideos' | 'tracks' | 'albums' | 'artists' | 'playlists';

interface SearchBatch {
  result?: SearchResult;
  type: SearchKind;
}

export default defineComponent({
  name: 'Search',
  components: {
    TrackList,
    MvRow,
    CoverRow,
  },
  data() {
    return {
      show: false,
      tracks: [] as Track[],
      artists: [] as Artist[],
      albums: [] as Album[],
      playlists: [] as Playlist[],
      musicVideos: [] as MusicVideo[],
    };
  },
  computed: {
    ...mapState(useAppStore, ['player']),
    keywords(): string {
      return String(this.$route.params['keywords'] ?? '');
    },
    haveResult() {
      return (
        this.tracks.length +
          this.artists.length +
          this.albums.length +
          this.playlists.length +
          this.musicVideos.length >
        0
      );
    },
  },
  watch: {
    keywords(newKeywords: string) {
      if (newKeywords.length === 0) return;
      this.getData();
    },
  },
  created() {
    this.getData();
  },
  methods: {
    ...mapActions(useAppStore, ['showToast']),
    search(type: SearchKind): Promise<SearchBatch> {
      const typeTable: Record<SearchKind, number> = {
        musicVideos: 1004,
        tracks: 1,
        albums: 10,
        artists: 100,
        playlists: 1000,
      };
      return searchApi({
        keywords: this.keywords,
        type: typeTable[type],
        limit: 16,
      }).then(response =>
        response.result === undefined
          ? { type }
          : { result: response.result, type }
      );
    },
    getData() {
      setTimeout(() => {
        if (!this.show) NProgress.start();
      }, 1000);
      this.show = false;

      const requestAll = (requests: Array<Promise<SearchBatch>>) => {
        const keywords = this.keywords;
        settleIndependentRequests(requests)
          .then(({ values, errors }) => {
            if (keywords != this.keywords) return;
            values.forEach(batch => {
              const searchType = batch.type;
              const result = batch.result;
              if (result === undefined) return;
              switch (searchType) {
                case 'musicVideos':
                  this.musicVideos = result.mvs ?? [];
                  break;
                case 'artists':
                  this.artists = result.artists ?? [];
                  break;
                case 'albums':
                  this.albums = result.albums ?? [];
                  break;
                case 'tracks':
                  this.tracks = result.songs ?? [];
                  this.getTracksDetail();
                  break;
                case 'playlists':
                  this.playlists = result.playlists ?? [];
                  break;
              }
            });
            NProgress.done();
            this.show = true;
            if (errors.length > 0) {
              this.showToast(`部分搜索失败：${String(errors[0])}`);
            }
          })
          .catch((error: unknown) => {
            NProgress.done();
            this.showToast(`搜索失败：${String(error)}`);
          });
      };

      const requests = [
        this.search('artists'),
        this.search('albums'),
        this.search('tracks'),
      ];
      const requests2 = [this.search('musicVideos'), this.search('playlists')];

      requestAll(requests);
      requestAll(requests2);
    },
    getTracksDetail() {
      const trackIDs = this.tracks.map(t => t.id);
      if (trackIDs.length === 0) return;
      getTrackDetail(trackIDs.join(',')).then(result => {
        this.tracks = result.songs;
      });
    },
  },
});
</script>

<style lang="scss" scoped>
.section-title {
  font-weight: 600;
  font-size: 22px;
  opacity: 0.88;
  color: var(--color-text);
  margin-bottom: 16px;

  display: flex;
  justify-content: space-between;
  align-items: center;
  a {
    font-size: 13px;
    font-weight: 600;
    opacity: 0.68;
  }
}

.row {
  display: flex;
  flex-wrap: wrap;
  margin-top: 32px;

  .artists {
    flex: 1;
    margin-right: 8rem;
  }
  .albums {
    flex: 1;
  }
}

.tracks,
.music-videos,
.playlists {
  margin-top: 46px;
}

.no-results {
  position: absolute;
  top: 64px;
  right: 0;
  left: 0;
  bottom: 64px;
  font-size: 24px;
  color: var(--color-text);
  opacity: 0.38;
  display: flex;
  justify-content: center;
  align-items: center;
  div {
    display: flex;
    align-items: center;
  }
  .svg-icon {
    height: 24px;
    width: 24px;
    margin-right: 16px;
  }
}
</style>
