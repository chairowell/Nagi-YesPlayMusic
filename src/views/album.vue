<template>
  <div v-show="show" class="album-page">
    <div class="playlist-info">
      <Cover
        :id="album.id"
        :image-url="$filters.resizeImage(album.picUrl, 1024)"
        :show-play-button="true"
        :always-show-shadow="true"
        :click-cover-to-play="true"
        :fixed-size="288"
        type="album"
        :cover-hover="false"
        :play-button-size="18"
        @click.right="openMenu"
      />
      <div class="info">
        <div class="title" @click.right="openMenu"> {{ title }}</div>
        <div v-if="subtitle !== ''" class="subtitle" @click.right="openMenu">{{
          subtitle
        }}</div>
        <div class="artist">
          <span v-if="album.artist.id !== 104700">
            <span>{{ $filters.formatAlbumType(album.type, album) }} by </span
            ><router-link :to="`/artist/${album.artist.id}`">{{
              album.artist.name
            }}</router-link></span
          >
          <span v-else>Compilation by Various Artists</span>
        </div>
        <div class="date-and-count">
          <span
            v-if="((album.mark ?? 0) & 1048576) === 1048576"
            class="explicit-symbol"
            ><ExplicitSymbol
          /></span>
          <span :title="$filters.formatDate(album.publishTime)">{{
            new Date(album.publishTime ?? 0).getFullYear()
          }}</span>
          <span> · {{ album.size }} {{ $t('common.songs') }}</span
          >,
          {{ $filters.formatTime(albumTime, 'Human') }}
        </div>
        <div class="description" @click="toggleFullDescription">
          {{ album.description }}
        </div>
        <div class="buttons" style="margin-top: 32px">
          <ButtonTwoTone icon-class="play" @click="playAlbumByID(album.id)">
            {{ $t('common.play') }}
          </ButtonTwoTone>
          <ButtonTwoTone
            :icon-class="dynamicDetail.isSub ? 'heart-solid' : 'heart'"
            :icon-button="true"
            :horizontal-padding="0"
            :color="dynamicDetail.isSub ? 'blue' : 'grey'"
            :text-color="dynamicDetail.isSub ? '#335eea' : ''"
            :background-color="
              dynamicDetail.isSub ? 'var(--color-secondary-bg)' : ''
            "
            @click="likeAlbum"
          >
          </ButtonTwoTone>
          <ButtonTwoTone
            icon-class="more"
            :icon-button="true"
            :horizontal-padding="0"
            color="grey"
            @click="openMenu"
          >
          </ButtonTwoTone>
        </div>
      </div>
    </div>
    <div v-if="tracksByDisc.length > 1">
      <div v-for="item in tracksByDisc" :key="item.disc">
        <h2 class="disc">Disc {{ item.disc }}</h2>
        <TrackList
          :id="album.id"
          :tracks="item.tracks"
          :type="'album'"
          :album-object="album"
        />
      </div>
    </div>
    <div v-else>
      <TrackList
        :id="album.id"
        :tracks="tracks"
        :type="'album'"
        :album-object="album"
      />
    </div>
    <div class="extra-info">
      <div class="album-time"></div>
      <div class="release-date">
        {{ $t('album.released') }}
        {{ $filters.formatDate(album.publishTime, 'MMMM D, YYYY') }}
      </div>
      <div v-if="album.company" class="copyright"> © {{ album.company }} </div>
    </div>
    <div v-if="filteredMoreAlbums.length !== 0" class="more-by">
      <div class="section-title">
        More by
        <router-link :to="`/artist/${album.artist.id}`"
          >{{ album.artist.name }}
        </router-link>
      </div>
      <div>
        <CoverRow
          type="album"
          :items="filteredMoreAlbums"
          sub-text="albumType+releaseYear"
        />
      </div>
    </div>
    <Modal
      :show="showFullDescription"
      :close="toggleFullDescription"
      :show-footer="false"
      :click-outside-hide="true"
      :title="$t('album.albumDesc')"
    >
      <p class="description-fulltext">
        {{ album.description }}
      </p>
    </Modal>
    <ContextMenu ref="albumMenu">
      <!-- <div class="item">{{ $t('contextMenu.addToQueue') }}</div> -->
      <div class="item" @click="likeAlbum(true)">{{
        dynamicDetail.isSub
          ? $t('contextMenu.removeFromLibrary')
          : $t('contextMenu.saveToLibrary')
      }}</div>
      <div class="item">{{ $t('contextMenu.addToPlaylist') }}</div>
      <div class="item" @click="copyUrl(album.id)">{{
        $t('contextMenu.copyUrl')
      }}</div>
      <div class="item" @click="openInBrowser(album.id)">{{
        $t('contextMenu.openInBrowser')
      }}</div>
    </ContextMenu>
  </div>
</template>

<script lang="ts">
import { defineComponent } from 'vue';
import { mapActions, mapState } from 'pinia';
import { useAppStore } from '@/stores/app';
import { getArtistAlbum } from '@/api/artist';
import { getTrackDetail } from '@/api/track';
import { getAlbum, albumDynamicDetail, likeAAlbum } from '@/api/album';
import locale from '@/locale';
import { splitSoundtrackAlbumTitle, splitAlbumTitle } from '@/utils/common';
import NProgress from 'nprogress';
import { isAccountLoggedIn } from '@/utils/auth';
import { groupBy, toPairs, sortBy } from 'lodash';

import ExplicitSymbol from '@/components/ExplicitSymbol.vue';
import ButtonTwoTone from '@/components/ButtonTwoTone.vue';
import ContextMenu from '@/components/ContextMenu.vue';
import TrackList from '@/components/TrackList.vue';
import CoverRow from '@/components/CoverRow.vue';
import Cover from '@/components/Cover.vue';
import Modal from '@/components/Modal.vue';
import { openExternalUrlSafely } from '@/services/externalLinks';
import type { Album, Track } from '@/types/domain';
import type { DetailedAlbum } from '@/api/album';

function emptyAlbum(): DetailedAlbum {
  return {
    id: 0,
    picUrl: '',
    artist: { id: 0 },
  };
}

export default defineComponent({
  name: 'Album',
  components: {
    Cover,
    ButtonTwoTone,
    TrackList,
    ExplicitSymbol,
    CoverRow,
    Modal,
    ContextMenu,
  },
  beforeRouteUpdate(to, from, next) {
    this.show = false;
    this.loadData(to.params['id']);
    next();
  },
  data() {
    return {
      show: false,
      album: emptyAlbum(),
      tracks: [] as Track[],
      showFullDescription: false,
      moreAlbums: [] as Album[],
      dynamicDetail: { isSub: false },
      subtitle: '',
      title: '',
    };
  },
  computed: {
    ...mapState(useAppStore, ['player', 'data']),
    albumTime() {
      let time = 0;
      this.tracks.forEach(track => (time += track.dt ?? 0));
      return time;
    },
    filteredMoreAlbums() {
      const moreAlbums = this.moreAlbums.filter(a => a.id !== this.album.id);
      const realAlbums = moreAlbums.filter(a => a.type === '专辑');
      const eps = moreAlbums.filter(
        a => a.type === 'EP' || (a.type === 'EP/Single' && (a.size ?? 0) > 1)
      );
      const restItems = moreAlbums.filter(
        a =>
          realAlbums.find(a1 => a1.id === a.id) === undefined &&
          eps.find(a1 => a1.id === a.id) === undefined
      );
      if (realAlbums.length === 0) {
        return [...realAlbums, ...eps, ...restItems].slice(0, 5);
      } else {
        return [...realAlbums, ...restItems].slice(0, 5);
      }
    },
    tracksByDisc() {
      if (this.tracks.length <= 1) return [];
      const pairs = toPairs(groupBy(this.tracks, 'cd'));
      return sortBy(pairs, p => p[0]).map(items => ({
        disc: items[0],
        tracks: items[1],
      }));
    },
  },
  created() {
    this.loadData(this.$route.params['id']);
  },
  methods: {
    ...mapActions(useAppStore, ['showToast', 'enableScrollingWith']),
    playAlbumByID(id: number, trackID: number | 'first' = 'first') {
      this.player.playAlbumByID(id, trackID);
    },
    likeAlbum(toast = false) {
      if (!isAccountLoggedIn()) {
        this.showToast(locale.t('toast.needToLogin'));
        return;
      }
      likeAAlbum({
        id: this.album.id,
        t: this.dynamicDetail.isSub ? 0 : 1,
      })
        .then(data => {
          if (data.code === 200) {
            this.dynamicDetail.isSub = !this.dynamicDetail.isSub;
            if (toast === true)
              this.showToast(
                this.dynamicDetail.isSub ? '已保存到音乐库' : '已从音乐库删除'
              );
          }
        })
        .catch((error: unknown) => {
          this.showToast(String(error));
        });
    },
    formatTitle() {
      const splitTitle = splitSoundtrackAlbumTitle(this.album.name ?? '');
      const splitTitle2 = splitAlbumTitle(splitTitle.title);
      this.title = splitTitle2.title;
      if (splitTitle.subtitle !== '' && splitTitle2.subtitle !== '') {
        this.subtitle = splitTitle.subtitle + ' · ' + splitTitle2.subtitle;
      } else {
        this.subtitle =
          splitTitle.subtitle === ''
            ? splitTitle2.subtitle
            : splitTitle.subtitle;
      }
    },
    loadData(id: unknown) {
      const albumId = Number(id);
      if (!Number.isFinite(albumId)) return;
      setTimeout(() => {
        if (!this.show) NProgress.start();
      }, 1000);
      getAlbum(albumId).then(data => {
        this.album = data.album;
        this.tracks = data.songs;
        this.formatTitle();
        NProgress.done();
        this.show = true;

        // to get explicit mark
        const trackIDs = this.tracks.map(t => t.id);
        getTrackDetail(trackIDs.join(',')).then(data => {
          this.tracks = data.songs;
        });

        // get more album by this artist
        getArtistAlbum({ id: this.album.artist.id, limit: 100 }).then(data => {
          this.moreAlbums = data.hotAlbums;
        });
      });
      albumDynamicDetail(albumId).then(data => {
        this.dynamicDetail = data;
      });
    },
    toggleFullDescription() {
      this.showFullDescription = !this.showFullDescription;
      if (this.showFullDescription) {
        this.enableScrollingWith(false);
      } else {
        this.enableScrollingWith(true);
      }
    },
    openMenu(e: MouseEvent) {
      (this.$refs['albumMenu'] as InstanceType<typeof ContextMenu>).openMenu(e);
    },
    copyUrl(id: number) {
      const showToast = this.showToast;
      this.$copyText(`https://music.163.com/#/album?id=${id}`)
        .then(function () {
          showToast(locale.t('toast.copied'));
        })
        .catch((error: unknown) => {
          showToast(`${locale.t('toast.copyFailed')}${String(error)}`);
        });
    },
    openInBrowser(id: number) {
      const url = `https://music.163.com/#/album?id=${id}`;
      void openExternalUrlSafely(url);
    },
  },
});
</script>

<style lang="scss" scoped>
.album-page {
  margin-top: 32px;
}
.playlist-info {
  display: flex;
  width: 78vw;
  margin-bottom: 72px;
  .info {
    display: flex;
    flex-direction: column;
    justify-content: center;
    flex: 1;
    margin-left: 56px;
    color: var(--color-text);
    .title {
      font-size: 56px;
      font-weight: 700;
    }
    .subtitle {
      font-size: 22px;
      font-weight: 600;
    }
    .artist {
      font-size: 18px;
      opacity: 0.88;
      margin-top: 24px;
      a {
        font-weight: 600;
      }
    }
    .date-and-count {
      font-size: 14px;
      opacity: 0.68;
      margin-top: 2px;
    }
    .description {
      user-select: none;
      font-size: 14px;
      opacity: 0.68;
      margin-top: 24px;
      display: -webkit-box;
      -webkit-box-orient: vertical;
      -webkit-line-clamp: 3;
      overflow: hidden;
      cursor: pointer;
      white-space: pre-line;
      &:hover {
        transition: opacity 0.3s;
        opacity: 0.88;
      }
    }
    .buttons {
      margin-top: 32px;
      display: flex;
      button {
        margin-right: 16px;
      }
    }
  }
}
.disc {
  color: var(--color-text);
}

.explicit-symbol {
  opacity: 0.28;
  color: var(--color-text);
  margin-right: 4px;
  .svg-icon {
    margin-bottom: -3px;
  }
}

.extra-info {
  margin-top: 36px;
  margin-bottom: 36px;
  font-size: 12px;
  opacity: 0.48;
  color: var(--color-text);
  div {
    margin-bottom: 4px;
  }
  .album-time {
    opacity: 0.68;
  }
}

.more-by {
  border-top: 1px solid rgba(128, 128, 128, 0.18);

  padding-top: 22px;
  .section-title {
    font-size: 22px;
    font-weight: 600;
    opacity: 0.88;
    color: var(--color-text);
    margin-bottom: 20px;
  }
}
.description-fulltext {
  font-size: 16px;
  margin-top: 24px;
  display: -webkit-box;
  -webkit-box-orient: vertical;
  overflow: hidden;
  white-space: pre-line;
}
</style>
