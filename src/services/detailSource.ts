import { getAppStore } from '@/stores/accessor';
import { unlockParams } from '@/services/playbackSource';
import { adaptTrackItems } from '@/services/searchSource';
import type { Artist, Track } from '@/types/domain';
import type { DetailedAlbum } from '@/api/album';
import type { DetailedPlaylist, PlaylistDetailResponse } from '@/api/playlist';

/**
 * Detail pages through the sidecar (core::ncm, shared with the TUI).
 * Container metadata arrives verbatim from upstream; only the song lists
 * are adapted back onto the legacy `ar`/`al`/`dt` track shape. Playable
 * status stays with the callers in `src/api`, as before.
 */
async function fetchDetail(
  kind: 'playlist' | 'album' | 'artist',
  id: number
): Promise<Record<string, unknown>> {
  const params = unlockParams(getAppStore().settings);
  params.set('id', String(id));
  const response = await fetch(`/api/native/${kind}/detail?${params}`);
  if (!response.ok) {
    throw new Error(`详情请求失败（HTTP ${response.status}）`);
  }
  const payload: unknown = await response.json();
  if (typeof payload !== 'object' || payload === null) {
    throw new Error('详情响应格式无效');
  }
  return payload as Record<string, unknown>;
}

function metaOf(
  body: Record<string, unknown>,
  field: string
): Record<string, unknown> {
  const meta = body[field];
  if (typeof meta !== 'object' || meta === null) {
    throw new Error(`详情响应缺少 ${field}`);
  }
  return meta as Record<string, unknown>;
}

function songsOf(body: Record<string, unknown>, field: string): Track[] {
  const items = Array.isArray(body[field])
    ? (body[field] as unknown[]).filter(
        (item): item is Record<string, unknown> =>
          typeof item === 'object' && item !== null
      )
    : [];
  return adaptTrackItems(items);
}

export async function fetchPlaylistDetail(
  id: number
): Promise<PlaylistDetailResponse> {
  const body = await fetchDetail('playlist', id);
  const playlist = {
    ...metaOf(body, 'playlist'),
    tracks: songsOf(body, 'songs'),
  } as unknown as DetailedPlaylist;
  return { playlist };
}

export async function fetchAlbumDetail(
  id: number
): Promise<{ album: DetailedAlbum; songs: Track[] }> {
  const body = await fetchDetail('album', id);
  return {
    album: metaOf(body, 'album') as unknown as DetailedAlbum,
    songs: songsOf(body, 'songs'),
  };
}

export async function fetchArtistDetail(
  id: number
): Promise<{ artist: Artist; hotSongs: Track[] }> {
  const body = await fetchDetail('artist', id);
  return {
    artist: metaOf(body, 'artist') as unknown as Artist,
    hotSongs: songsOf(body, 'hotSongs'),
  };
}
