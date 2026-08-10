import router from '../router';
import { getAppStore } from '@/stores/accessor';
import {
  recommendPlaylist,
  dailyRecommendPlaylist,
  getPlaylistDetail,
} from '@/api/playlist';
import { isAccountLoggedIn } from '@/utils/auth';
import type { Playlist } from '@/types/domain';

export function hasListSource(): boolean {
  const { player } = getAppStore();
  return !player.isPersonalFM && player.playlistSource.id !== 0;
}

export function goToListSource(): void {
  router.push({ path: getListSourcePath() });
}

export function getListSourcePath(): string {
  const { data, player } = getAppStore();
  if (player.playlistSource.id === data.likedSongPlaylistID) {
    return '/library/liked-songs';
  } else if (player.playlistSource.type === 'url') {
    return String(player.playlistSource.id);
  } else if (player.playlistSource.type === 'cloudDisk') {
    return '/library';
  } else {
    return `/${player.playlistSource.type}/${player.playlistSource.id}`;
  }
}

export async function getRecommendPlayList(
  limit: number,
  removePrivateRecommand: boolean
): Promise<Playlist[]> {
  if (isAccountLoggedIn()) {
    const playlists = await Promise.all([
      dailyRecommendPlaylist(),
      recommendPlaylist({ limit }),
    ]);
    let recommend = playlists[0].recommend ?? [];
    if (recommend.length) {
      if (removePrivateRecommand) recommend = recommend.slice(1);
      await replaceRecommendResult(recommend);
    }
    return recommend.concat(playlists[1].result).slice(0, limit);
  } else {
    const response = await recommendPlaylist({ limit });
    return response.result;
  }
}

async function replaceRecommendResult(recommend: Playlist[]): Promise<void> {
  for (const r of recommend) {
    if (specialPlaylist.indexOf(r.id) > -1) {
      const data = await getPlaylistDetail(r.id, true);
      const playlist = data.playlist;
      if (playlist) {
        if (playlist.name !== undefined) r.name = playlist.name;
        if (playlist.coverImgUrl !== undefined) {
          r.picUrl = playlist.coverImgUrl;
        }
      }
    }
  }
}

const specialPlaylist = [3136952023, 2829883282, 2829816518, 2829896389];
