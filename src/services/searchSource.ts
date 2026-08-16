import { getAppStore } from '@/stores/accessor';
import { unlockParams } from '@/services/playbackSource';
import { mapTrackPlayableStatus } from '@/utils/common';
import type {
  Album,
  Artist,
  MusicVideo,
  Playlist,
  Track,
  TrackPrivilege,
  UserProfile,
} from '@/types/domain';

export interface SearchPageOf<T> {
  items: T[];
  total: number;
}

interface SearchOptions {
  limit?: number;
  offset?: number;
}

/**
 * Typed search through the sidecar (core::ncm cloudsearch, shared with the
 * TUI). Each channel adapter maps the flat payload back onto the legacy
 * component item shapes so CoverRow/MvRow/TrackList stay unchanged.
 */
async function fetchSearch(
  keywords: string,
  type: number,
  options: SearchOptions
): Promise<{ total: number; items: Record<string, unknown>[] }> {
  const params = unlockParams(getAppStore().settings);
  params.set('keywords', keywords);
  params.set('type', String(type));
  if (options.limit !== undefined) params.set('limit', String(options.limit));
  if (options.offset !== undefined) {
    params.set('offset', String(options.offset));
  }
  const response = await fetch(`/api/native/search?${params}`);
  if (!response.ok) {
    throw new Error(`搜索请求失败（HTTP ${response.status}）`);
  }
  const payload: unknown = await response.json();
  if (typeof payload !== 'object' || payload === null) {
    throw new Error('搜索响应格式无效');
  }
  const body = payload as Record<string, unknown>;
  const items = Array.isArray(body['items'])
    ? (body['items'] as unknown[]).filter(
        (item): item is Record<string, unknown> =>
          typeof item === 'object' && item !== null
      )
    : [];
  return {
    total: typeof body['total'] === 'number' ? body['total'] : items.length,
    items,
  };
}

function optionalText(field: string, value: unknown): Record<string, string> {
  // Empty strings pass through: MvRow renders the literal 'null' for a
  // missing artistName but blank for an empty one, matching the old path.
  return typeof value === 'string' ? { [field]: value } : {};
}

export async function searchTracks(
  keywords: string,
  options: SearchOptions = {}
): Promise<SearchPageOf<Track>> {
  const page = await fetchSearch(keywords, 1, options);
  const items = page.items
    .filter(item => typeof item['id'] === 'number')
    .map(item => {
      const album =
        typeof item['album'] === 'object' && item['album'] !== null
          ? (item['album'] as Record<string, unknown>)
          : {};
      const track: Track = {
        id: item['id'] as number,
        ...optionalText('name', item['name']),
        ar: Array.isArray(item['artists']) ? (item['artists'] as Artist[]) : [],
        al: {
          id: typeof album['id'] === 'number' ? album['id'] : 0,
          ...optionalText('name', album['name']),
          ...optionalText('picUrl', album['picUrl']),
        },
        dt: typeof item['durationMs'] === 'number' ? item['durationMs'] : 0,
        alia: Array.isArray(item['alias']) ? (item['alias'] as string[]) : [],
        tns: Array.isArray(item['transNames'])
          ? (item['transNames'] as string[])
          : [],
        mark: typeof item['mark'] === 'number' ? item['mark'] : 0,
        ...(typeof item['fee'] === 'number' ? { fee: item['fee'] } : {}),
        // Presence alone marks "no copyright" downstream, so only set it
        // when the server says true.
        ...(item['noCopyrightRcmd'] === true ? { noCopyrightRcmd: true } : {}),
        ...(typeof item['privilege'] === 'object' && item['privilege'] !== null
          ? { privilege: item['privilege'] as TrackPrivilege }
          : {}),
      };
      return track;
    });
  return { items: mapTrackPlayableStatus(items), total: page.total };
}

export async function searchArtists(
  keywords: string,
  options: SearchOptions = {}
): Promise<SearchPageOf<Artist>> {
  const page = await fetchSearch(keywords, 100, options);
  return {
    items: page.items
      .filter(item => typeof item['id'] === 'number')
      .map(item => ({
        id: item['id'] as number,
        ...optionalText('name', item['name']),
        ...optionalText('picUrl', item['picUrl']),
        ...optionalText('img1v1Url', item['img1v1Url']),
      })),
    total: page.total,
  };
}

export async function searchAlbums(
  keywords: string,
  options: SearchOptions = {}
): Promise<SearchPageOf<Album>> {
  const page = await fetchSearch(keywords, 10, options);
  return {
    items: page.items
      .filter(item => typeof item['id'] === 'number')
      .map(item => ({
        id: item['id'] as number,
        ...optionalText('name', item['name']),
        ...optionalText('picUrl', item['picUrl']),
        ...(typeof item['artist'] === 'object' && item['artist'] !== null
          ? { artist: item['artist'] as Artist }
          : {}),
        mark: typeof item['mark'] === 'number' ? item['mark'] : 0,
      })),
    total: page.total,
  };
}

export async function searchPlaylists(
  keywords: string,
  options: SearchOptions = {}
): Promise<SearchPageOf<Playlist>> {
  const page = await fetchSearch(keywords, 1000, options);
  return {
    items: page.items
      .filter(item => typeof item['id'] === 'number')
      .map(item => ({
        id: item['id'] as number,
        ...optionalText('name', item['name']),
        ...optionalText('coverImgUrl', item['coverUrl']),
        privacy: typeof item['privacy'] === 'number' ? item['privacy'] : 0,
      })),
    total: page.total,
  };
}

export async function searchMusicVideos(
  keywords: string,
  options: SearchOptions = {}
): Promise<SearchPageOf<MusicVideo>> {
  const page = await fetchSearch(keywords, 1004, options);
  return {
    items: page.items
      .filter(item => typeof item['id'] === 'number')
      .map(item => ({
        id: item['id'] as number,
        ...optionalText('name', item['name']),
        ...optionalText('cover', item['coverUrl']),
        ...optionalText('artistName', item['artistName']),
        ...(typeof item['artistId'] === 'number'
          ? { artistId: item['artistId'] }
          : {}),
      })),
    total: page.total,
  };
}

export async function searchUsers(
  keywords: string,
  options: SearchOptions = {}
): Promise<SearchPageOf<UserProfile>> {
  const page = await fetchSearch(keywords, 1002, options);
  return {
    items: page.items
      .filter(item => typeof item['userId'] === 'number')
      .map(item => ({
        userId: item['userId'] as number,
        // The settings page badges on vipType !== 0, so absence must read 0.
        vipType: typeof item['vipType'] === 'number' ? item['vipType'] : 0,
        ...optionalText('nickname', item['nickname']),
        ...optionalText('avatarUrl', item['avatarUrl']),
        ...optionalText('signature', item['signature']),
      })),
    total: page.total,
  };
}
