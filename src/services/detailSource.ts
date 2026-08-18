import { getAppStore } from '@/stores/accessor';
import { nativeFetch } from '@/utils/nativeFetch';
import { unlockParams } from '@/services/playbackSource';
import { adaptTrackItems } from '@/services/songItems';
import { decodeTrackCollectionResponse } from '@/api/decoders';
import type { Artist, Track } from '@/types/domain';
import type { DetailedAlbum } from '@/api/album';
import type { DetailedPlaylist, PlaylistDetailResponse } from '@/api/playlist';
import type { TrackCollectionResponse } from '@/api/types';

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
  const response = await nativeFetch(`/api/native/${kind}/detail?${params}`);
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

/**
 * Metadata passes through verbatim, but the fields the pages dereference
 * unconditionally (the old decoders' guarantee) still gate the response:
 * a code-200 body missing them must fail the request, not crash a view.
 */
function requireSpine(checks: Record<string, boolean>, what: string): void {
  for (const [field, ok] of Object.entries(checks)) {
    if (!ok) {
      throw new Error(`${what}响应缺少 ${field}`);
    }
  }
}

export async function fetchPlaylistDetail(
  id: number
): Promise<PlaylistDetailResponse> {
  const body = await fetchDetail('playlist', id);
  const meta = metaOf(body, 'playlist');
  requireSpine(
    {
      id: typeof meta['id'] === 'number',
      creator: typeof meta['creator'] === 'object' && meta['creator'] !== null,
      trackIds: Array.isArray(meta['trackIds']),
    },
    '歌单详情'
  );
  const playlist = {
    ...meta,
    tracks: songsOf(body, 'songs'),
    // Pre-drop embedded row count: the playlist page's paging cursor
    // indexes trackIds, so it must count source rows, not survivors.
    ...(typeof body['embeddedCount'] === 'number'
      ? { embeddedTrackCount: body['embeddedCount'] }
      : {}),
  } as unknown as DetailedPlaylist;
  return { playlist };
}

export async function fetchAlbumDetail(
  id: number
): Promise<{ album: DetailedAlbum; songs: Track[] }> {
  const body = await fetchDetail('album', id);
  const meta = metaOf(body, 'album');
  requireSpine(
    {
      id: typeof meta['id'] === 'number',
      artist: typeof meta['artist'] === 'object' && meta['artist'] !== null,
    },
    '专辑详情'
  );
  return {
    album: meta as unknown as DetailedAlbum,
    songs: songsOf(body, 'songs'),
  };
}

/**
 * Verbatim `/song/detail` rows: the caller caches them in IndexedDB and
 * recomputes playability at read time, so the transport must not narrow
 * the shape — the strict legacy decoder still runs on the raw payload.
 */
export async function fetchTrackDetails(
  ids: string
): Promise<TrackCollectionResponse> {
  const params = unlockParams(getAppStore().settings);
  params.set('ids', ids);
  const response = await nativeFetch(`/api/native/song/detail?${params}`);
  if (!response.ok) {
    throw new Error(`歌曲详情请求失败（HTTP ${response.status}）`);
  }
  const payload: unknown = await response.json();
  return decodeTrackCollectionResponse(payload, {
    url: '/native/song/detail',
  });
}

export async function fetchArtistDetail(
  id: number
): Promise<{ artist: Artist; hotSongs: Track[] }> {
  const body = await fetchDetail('artist', id);
  const meta = metaOf(body, 'artist');
  requireSpine({ id: typeof meta['id'] === 'number' }, '歌手详情');
  return {
    artist: meta as unknown as Artist,
    hotSongs: songsOf(body, 'hotSongs'),
  };
}
