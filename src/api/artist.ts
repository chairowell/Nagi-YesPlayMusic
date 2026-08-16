import request from '@/utils/request';
import { mapTrackPlayableStatus } from '@/utils/common';
import { isAccountLoggedIn } from '@/utils/auth';
import { getTrackDetail } from '@/api/track';
import { fetchArtistDetail } from '@/services/detailSource';
import type { Album, Artist, MusicVideo, Track } from '@/types/domain';
import type { ApiResponse } from './types';
import {
  decodeAlbum,
  decodeArray,
  decodeArtist,
  decodeBoolean,
  decodeCodeResponse,
  decodeMusicVideo,
  decodeRecord,
} from './decoders';
import type { Decoder } from './decoders';

interface ArtistDetailResponse extends ApiResponse {
  artist: Artist;
  hotSongs: Track[];
}

interface ArtistAlbumsResponse extends ApiResponse {
  hotAlbums: Album[];
}

interface ArtistMvsResponse extends ApiResponse {
  mvs: MusicVideo[];
  hasMore: boolean;
}

const decodeArtistAlbumsResponse: Decoder<ArtistAlbumsResponse> = (
  input,
  context
) => {
  const response = decodeRecord(input, context);
  return {
    ...response,
    hotAlbums: decodeArray(
      response['hotAlbums'],
      context,
      '$.hotAlbums',
      decodeAlbum
    ),
  };
};

const decodeArtistToplistResponse: Decoder<
  ApiResponse & { list: { artists: Artist[] } }
> = (input, context) => {
  const response = decodeRecord(input, context);
  const list = decodeRecord(response['list'], context, '$.list');
  return {
    ...response,
    list: {
      ...list,
      artists: decodeArray(
        list['artists'],
        context,
        '$.list.artists',
        decodeArtist
      ),
    },
  };
};

const decodeArtistMvsResponse: Decoder<ArtistMvsResponse> = (
  input,
  context
) => {
  const response = decodeRecord(input, context);
  return {
    ...response,
    mvs: decodeArray(response['mvs'], context, '$.mvs', decodeMusicVideo),
    hasMore: decodeBoolean(response['hasMore'], context, '$.hasMore'),
  };
};

const decodeArtistsResponse: Decoder<ApiResponse & { artists: Artist[] }> = (
  input,
  context
) => {
  const response = decodeRecord(input, context);
  return {
    ...response,
    artists: decodeArray(
      response['artists'],
      context,
      '$.artists',
      decodeArtist
    ),
  };
};

export function getArtist(id: number): Promise<ArtistDetailResponse> {
  // Typed sidecar endpoint (core::ncm), shared with the TUI.
  return fetchArtistDetail(id).then(async (data: ArtistDetailResponse) => {
    if (!isAccountLoggedIn()) {
      const trackIDs = data.hotSongs.map(t => t.id);
      const tracks = await getTrackDetail(trackIDs.join(','));
      data.hotSongs = tracks.songs;
      return data;
    }
    data.hotSongs = mapTrackPlayableStatus(data.hotSongs);
    return data;
  });
}

export function getArtistAlbum(params: {
  id: number;
  limit?: number;
  offset?: number;
}): Promise<ArtistAlbumsResponse> {
  return request<ArtistAlbumsResponse>(
    {
      url: '/artist/album',
      method: 'get',
      params,
    },
    decodeArtistAlbumsResponse
  );
}

export function toplistOfArtists(
  type: number | null = null
): Promise<ApiResponse & { list: { artists: Artist[] } }> {
  const params: { type?: number } = {};
  if (type) {
    params.type = type;
  }
  return request<ApiResponse & { list: { artists: Artist[] } }>(
    {
      url: '/toplist/artist',
      method: 'get',
      params,
    },
    decodeArtistToplistResponse
  );
}
export function artistMv(params: {
  id: number;
  offset?: number;
  limit?: number;
}): Promise<ArtistMvsResponse> {
  return request<ArtistMvsResponse>(
    {
      url: '/artist/mv',
      method: 'get',
      params,
    },
    decodeArtistMvsResponse
  );
}

export function followAArtist(params: {
  id: number;
  t: number;
}): Promise<ApiResponse & { code: number }> {
  return request<ApiResponse & { code: number }>(
    {
      url: '/artist/sub',
      method: 'post',
      params,
    },
    decodeCodeResponse
  );
}

export function similarArtists(
  id: number
): Promise<ApiResponse & { artists: Artist[] }> {
  return request<ApiResponse & { artists: Artist[] }>(
    {
      url: '/simi/artist',
      method: 'post',
      params: { id },
    },
    decodeArtistsResponse
  );
}
