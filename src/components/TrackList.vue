<template>
  <div class="track-list">
    <ContextMenu ref="menu" @close="closeMenu">
      <div v-show="type !== 'cloudDisk'" class="item-info">
        <img
          :src="$filters.resizeImage(rightClickedTrackComputed.al?.picUrl, 224)"
          loading="lazy"
        />
        <div class="info">
          <div class="title">{{ rightClickedTrackComputed.name }}</div>
          <div class="subtitle">{{
            rightClickedTrackComputed.ar?.[0]?.name ?? ''
          }}</div>
        </div>
      </div>
      <hr v-show="type !== 'cloudDisk'" />
      <div class="item" @click="play">{{ $t('contextMenu.play') }}</div>
      <div class="item" @click="addToQueue">{{
        $t('contextMenu.addToQueue')
      }}</div>
      <div
        v-if="extraContextMenuItem.includes('removeTrackFromQueue')"
        class="item"
        @click="removeTrackFromQueue"
        >从队列删除</div
      >
      <hr v-show="type !== 'cloudDisk'" />
      <div
        v-show="!isRightClickedTrackLiked && type !== 'cloudDisk'"
        class="item"
        @click="like"
      >
        {{ $t('contextMenu.saveToMyLikedSongs') }}
      </div>
      <div
        v-show="isRightClickedTrackLiked && type !== 'cloudDisk'"
        class="item"
        @click="like"
      >
        {{ $t('contextMenu.removeFromMyLikedSongs') }}
      </div>
      <div
        v-if="extraContextMenuItem.includes('removeTrackFromPlaylist')"
        class="item"
        @click="removeTrackFromPlaylist"
        >从歌单中删除</div
      >
      <div
        v-show="type !== 'cloudDisk'"
        class="item"
        @click="addTrackToPlaylist"
        >{{ $t('contextMenu.addToPlaylist') }}</div
      >
      <div v-show="type !== 'cloudDisk'" class="item" @click="copyLink">{{
        $t('contextMenu.copyUrl')
      }}</div>
      <div
        v-if="extraContextMenuItem.includes('removeTrackFromCloudDisk')"
        class="item"
        @click="removeTrackFromCloudDisk"
        >从云盘中删除</div
      >
    </ContextMenu>

    <div :style="listStyles">
      <TrackListItem
        v-for="(track, index) in tracks"
        :key="itemKey === 'id' ? track.id : `${track.id}${index}`"
        :track-prop="track"
        :track-no="index + 1"
        :track-type="type"
        :album-artist-name="albumObject.artist?.name ?? ''"
        :liked-song-ids="liked.songs"
        :context-track-id="rightClickedTrack.id"
        :highlight-playing-track="highlightPlayingTrack"
        @play-track="playThisList"
        @like-track="likeATrack"
        @dblclick="playThisList(track.id || track.songId)"
        @click.right="openMenu($event, track, index)"
      />
    </div>
  </div>
</template>

<script lang="ts">
import { defineComponent } from 'vue';
import type { CSSProperties, PropType } from 'vue';
import { mapActions, mapState } from 'pinia';
import { useAppStore } from '@/stores/app';
import { addOrRemoveTrackFromPlaylist } from '@/api/playlist';
import { cloudDiskTrackDelete } from '@/api/user';
import { isAccountLoggedIn } from '@/utils/auth';

import TrackListItem from '@/components/TrackListItem.vue';
import ContextMenu from '@/components/ContextMenu.vue';
import locale from '@/locale';
import type { Album, Track } from '@/types/domain';

type TrackListType = 'tracklist' | 'album' | 'playlist' | 'cloudDisk';
type DoubleClickAction =
  | 'default'
  | 'none'
  | 'playTrackOnListByID'
  | 'playQueuedTrackByID'
  | 'playPlaylistByID'
  | 'playAList'
  | 'dailyTracks'
  | 'playCloudDisk';
type ExtraContextAction =
  | 'removeTrackFromPlaylist'
  | 'removeTrackFromQueue'
  | 'removeTrackFromCloudDisk';

const emptyTrack = (): Track => ({
  id: 0,
  name: '',
  ar: [{ id: 0, name: '' }],
  al: { id: 0, picUrl: '' },
});

export default defineComponent({
  name: 'TrackList',
  components: {
    TrackListItem,
    ContextMenu,
  },
  emits: {
    'remove-track': (trackId: number) => Number.isFinite(trackId),
  },
  props: {
    tracks: {
      type: Array as PropType<Track[]>,
      default: () => [],
    },
    type: {
      type: String as PropType<TrackListType>,
      default: 'tracklist',
    }, // tracklist | album | playlist | cloudDisk
    id: {
      type: Number,
      default: 0,
    },
    dbclickTrackFunc: {
      type: String as PropType<DoubleClickAction>,
      default: 'default',
    },
    albumObject: {
      type: Object as PropType<Album>,
      default: () => {
        return {
          artist: {
            id: 0,
            name: '',
          },
        };
      },
    },
    extraContextMenuItem: {
      type: Array as PropType<ExtraContextAction[]>,
      default: () => [],
    },
    columnNumber: {
      type: Number,
      default: 4,
    },
    highlightPlayingTrack: {
      type: Boolean,
      default: true,
    },
    itemKey: {
      type: String as PropType<'id' | 'id+index'>,
      default: 'id',
    },
  },
  data() {
    return {
      rightClickedTrack: emptyTrack(),
      rightClickedTrackIndex: -1,
      listStyles: {} as CSSProperties,
    };
  },
  computed: {
    ...mapState(useAppStore, ['liked', 'player']),
    isRightClickedTrackLiked() {
      return this.liked.songs.includes(this.rightClickedTrack?.id);
    },
    rightClickedTrackComputed() {
      return this.type === 'cloudDisk'
        ? {
            id: 0,
            name: '',
            ar: [{ name: '' }],
            al: { picUrl: '' },
          }
        : this.rightClickedTrack;
    },
  },
  created() {
    if (this.type === 'tracklist') {
      this.listStyles = {
        display: 'grid',
        gap: '4px',
        gridTemplateColumns: `repeat(${this.columnNumber}, 1fr)`,
      };
    }
  },
  methods: {
    ...mapActions(useAppStore, [
      'updateModal',
      'updateLikedXXX',
      'showToast',
      'likeATrack',
    ]),
    openMenu(e: MouseEvent, track: Track, index = -1) {
      this.rightClickedTrack = track;
      this.rightClickedTrackIndex = index;
      (this.$refs['menu'] as InstanceType<typeof ContextMenu>).openMenu(e);
    },
    closeMenu() {
      this.rightClickedTrack = emptyTrack();
      this.rightClickedTrackIndex = -1;
    },
    playThisList(trackID: number | undefined) {
      if (trackID === undefined) return;
      if (this.dbclickTrackFunc === 'default') {
        this.playThisListDefault(trackID);
      } else if (this.dbclickTrackFunc === 'none') {
        // do nothing
      } else if (this.dbclickTrackFunc === 'playTrackOnListByID') {
        this.player.playTrackOnListByID(trackID);
      } else if (this.dbclickTrackFunc === 'playQueuedTrackByID') {
        this.player.playTrackOnListByID(trackID, 'playNext');
      } else if (this.dbclickTrackFunc === 'playPlaylistByID') {
        this.player.playPlaylistByID(this.id, trackID);
      } else if (this.dbclickTrackFunc === 'playAList') {
        const trackIDs = this.tracks
          .map(t => t.id || t.songId)
          .filter((id): id is number => id !== undefined);
        this.player.replacePlaylist(trackIDs, this.id, 'artist', trackID);
      } else if (this.dbclickTrackFunc === 'dailyTracks') {
        const trackIDs = this.tracks.map(t => t.id);
        this.player.replacePlaylist(trackIDs, '/daily/songs', 'url', trackID);
      } else if (this.dbclickTrackFunc === 'playCloudDisk') {
        const trackIDs = this.tracks
          .map(t => t.id || t.songId)
          .filter((id): id is number => id !== undefined);
        this.player.replacePlaylist(trackIDs, this.id, 'cloudDisk', trackID);
      }
    },
    playThisListDefault(trackID: number) {
      if (this.type === 'playlist') {
        this.player.playPlaylistByID(this.id, trackID);
      } else if (this.type === 'album') {
        this.player.playAlbumByID(this.id, trackID);
      } else if (this.type === 'tracklist') {
        const trackIDs = this.tracks.map(t => t.id);
        this.player.replacePlaylist(trackIDs, this.id, 'artist', trackID);
      }
    },
    play() {
      this.player.addTrackToPlayNext(this.rightClickedTrack.id, true);
    },
    addToQueue() {
      this.player.addTrackToPlayNext(this.rightClickedTrack.id);
    },
    like() {
      this.likeATrack(this.rightClickedTrack.id);
    },
    addTrackToPlaylist() {
      if (!isAccountLoggedIn()) {
        this.showToast(locale.t('toast.needToLogin'));
        return;
      }
      this.updateModal({
        modalName: 'addTrackToPlaylistModal',
        key: 'show',
        value: true,
      });
      this.updateModal({
        modalName: 'addTrackToPlaylistModal',
        key: 'selectedTrackID',
        value: this.rightClickedTrack.id,
      });
    },
    removeTrackFromPlaylist() {
      if (!isAccountLoggedIn()) {
        this.showToast(locale.t('toast.needToLogin'));
        return;
      }
      if (confirm(`确定要从歌单删除 ${this.rightClickedTrack.name}？`)) {
        const trackID = this.rightClickedTrack.id;
        addOrRemoveTrackFromPlaylist({
          op: 'del',
          pid: this.id,
          tracks: trackID,
        }).then(data => {
          this.showToast(
            data.body.code === 200
              ? locale.t('toast.removedFromPlaylist')
              : data.body.message ?? '删除歌曲失败'
          );
          this.$emit('remove-track', trackID);
        });
      }
    },
    copyLink() {
      this.$copyText(
        `https://music.163.com/song?id=${this.rightClickedTrack.id}`
      )
        .then(() => {
          this.showToast(locale.t('toast.copied'));
        })
        .catch((error: unknown) => {
          this.showToast(`${locale.t('toast.copyFailed')}${String(error)}`);
        });
    },
    removeTrackFromQueue() {
      this.player.removeTrackFromQueue(this.rightClickedTrackIndex);
    },
    removeTrackFromCloudDisk() {
      if (confirm(`确定要从云盘删除 ${this.rightClickedTrack.songName}？`)) {
        const trackID = this.rightClickedTrack.songId;
        if (trackID === undefined) return;
        cloudDiskTrackDelete(trackID).then(data => {
          this.showToast(
            data.code === 200
              ? '已将此歌曲从云盘删除'
              : data.message ?? '删除失败'
          );
          const newCloudDisk = this.liked.cloudDisk.filter(
            t => t.songId !== trackID
          );
          this.updateLikedXXX({
            name: 'cloudDisk',
            data: newCloudDisk,
          });
        });
      }
    },
  },
});
</script>

<style lang="scss" scoped></style>
