import request from '@/utils/request';
import { mapTrackPlayableStatus } from '@/utils/common';
import { fetchPlaylistDetail } from '@/services/detailSource';
import type {
  Playlist,
  Track,
  TrackPrivilege,
  UserProfile,
} from '@/types/domain';
import type { ApiResponse } from './types';
import {
  decodeApiResponse,
  decodeArray,
  decodeBoolean,
  decodeCodeResponse,
  decodeNumber,
  decodeOptionalString,
  decodePlaylist,
  decodeRecord,
  decodeTrack,
} from './decoders';
import type { Decoder } from './decoders';

export interface DetailedPlaylist extends Playlist {
  creator: UserProfile;
  trackIds: Array<{ id: number }>;
  tracks: Track[];
}

export interface PlaylistDetailResponse extends ApiResponse {
  playlist?: DetailedPlaylist;
  privileges?: TrackPrivilege[];
}

export interface PlaylistMutationResponse extends ApiResponse {
  body: {
    code: number;
    message?: string;
  };
}

export interface CreatePlaylistResponse extends ApiResponse {
  code: number;
  id: number;
}

const decodePlaylistResultResponse: Decoder<
  ApiResponse & { result: Playlist[] }
> = (input, context) => {
  const response = decodeRecord(input, context);
  return {
    ...response,
    result: decodeArray(
      response['result'],
      context,
      '$.result',
      decodePlaylist
    ),
  };
};

const decodePlaylistRecommendResponse: Decoder<
  ApiResponse & { recommend: Playlist[] }
> = (input, context) => {
  const response = decodeRecord(input, context);
  return {
    ...response,
    recommend: decodeArray(
      response['recommend'],
      context,
      '$.recommend',
      decodePlaylist
    ),
  };
};

const decodePlaylistsResponse: Decoder<
  ApiResponse & { playlists: Playlist[]; more: boolean }
> = (input, context) => {
  const response = decodeRecord(input, context);
  return {
    ...response,
    playlists: decodeArray(
      response['playlists'],
      context,
      '$.playlists',
      decodePlaylist
    ),
    more: decodeBoolean(response['more'], context, '$.more'),
  };
};

const decodePlaylistListResponse: Decoder<
  ApiResponse & { list: Playlist[] }
> = (input, context) => {
  const response = decodeRecord(input, context);
  return {
    ...response,
    list: decodeArray(response['list'], context, '$.list', decodePlaylist),
  };
};

const decodeCreatePlaylistResponse: Decoder<CreatePlaylistResponse> = (
  input,
  context
) => {
  const response = decodeRecord(input, context);
  return {
    ...response,
    code: decodeNumber(response['code'], context, '$.code'),
    id: decodeNumber(response['id'], context, '$.id'),
  };
};

const decodePlaylistMutationResponse: Decoder<PlaylistMutationResponse> = (
  input,
  context
) => {
  const response = decodeRecord(input, context);
  const body = decodeRecord(response['body'], context, '$.body');
  const message = decodeOptionalString(
    body['message'],
    context,
    '$.body.message'
  );
  return {
    ...response,
    body: {
      ...body,
      code: decodeNumber(body['code'], context, '$.body.code'),
      ...(message === undefined ? {} : { message }),
    },
  };
};

const decodeTrackDataResponse: Decoder<ApiResponse & { data: Track[] }> = (
  input,
  context
) => {
  const response = decodeRecord(input, context);
  return {
    ...response,
    data: decodeArray(response['data'], context, '$.data', decodeTrack),
  };
};

export function recommendPlaylist(params: { limit?: number }) {
  return request<ApiResponse & { result: Playlist[] }>(
    {
      url: '/personalized',
      method: 'get',
      params,
    },
    decodePlaylistResultResponse
  );
}
export function dailyRecommendPlaylist(params: { limit?: number } = {}) {
  return request<ApiResponse & { recommend: Playlist[] }>(
    {
      url: '/recommend/resource',
      method: 'get',
      params: {
        params,
        timestamp: Date.now(),
      },
    },
    decodePlaylistRecommendResponse
  );
}
export function getPlaylistDetail(
  id: number,
  noCache = false
): Promise<PlaylistDetailResponse> {
  // Typed sidecar endpoint (core::ncm), never cached, so the old
  // timestamp cache-buster flag has nothing left to bust.
  void noCache;
  return fetchPlaylistDetail(id).then(data => {
    if (data.playlist) {
      // Privileges arrive merged into each track by the core.
      data.playlist.tracks = mapTrackPlayableStatus(data.playlist.tracks ?? []);
    }
    return data;
  });
}
export function highQualityPlaylist(params: {
  cat?: string;
  limit?: number;
  before: number;
}): Promise<ApiResponse & { playlists: Playlist[]; more: boolean }> {
  return request<ApiResponse & { playlists: Playlist[]; more: boolean }>(
    {
      url: '/top/playlist/highquality',
      method: 'get',
      params,
    },
    decodePlaylistsResponse
  );
}

export function topPlaylist(params: {
  order?: string;
  cat: string;
  limit?: number;
  offset?: number;
}): Promise<ApiResponse & { playlists: Playlist[]; more: boolean }> {
  return request<ApiResponse & { playlists: Playlist[]; more: boolean }>(
    {
      url: '/top/playlist',
      method: 'get',
      params,
    },
    decodePlaylistsResponse
  );
}

export function playlistCatlist() {
  return request<ApiResponse>(
    {
      url: '/playlist/catlist',
      method: 'get',
    },
    decodeApiResponse
  );
}

export function toplists() {
  return request<ApiResponse & { list: Playlist[] }>(
    {
      url: '/toplist',
      method: 'get',
    },
    decodePlaylistListResponse
  );
}

export function subscribePlaylist(params: {
  t: number;
  id: number;
  timestamp?: number;
}): Promise<ApiResponse & { code: number }> {
  params.timestamp = new Date().getTime();
  return request<ApiResponse & { code: number }>(
    {
      url: '/playlist/subscribe',
      method: 'post',
      params,
    },
    decodeCodeResponse
  );
}

export function deletePlaylist(
  id: number | string
): Promise<ApiResponse & { code: number }> {
  return request<ApiResponse & { code: number }>(
    {
      url: '/playlist/delete',
      method: 'post',
      params: { id },
    },
    decodeCodeResponse
  );
}

export function createPlaylist(params: {
  name: string;
  privacy?: 10;
  type?: 'NORMAL' | 'VIDEO';
  timestamp?: number;
}): Promise<CreatePlaylistResponse> {
  params.timestamp = new Date().getTime();
  return request<CreatePlaylistResponse>(
    {
      url: '/playlist/create',
      method: 'post',
      params,
    },
    decodeCreatePlaylistResponse
  );
}

export function addOrRemoveTrackFromPlaylist(params: {
  op: string;
  pid: string | number;
  tracks?: string | number;
  timestamp?: number;
}): Promise<PlaylistMutationResponse> {
  params.timestamp = new Date().getTime();
  return request<PlaylistMutationResponse>(
    {
      url: '/playlist/tracks',
      method: 'post',
      params,
    },
    decodePlaylistMutationResponse
  );
}

export function intelligencePlaylist(params: {
  id?: number;
  pid?: number;
  sid?: number;
}): Promise<ApiResponse & { data: Track[] }> {
  return request<ApiResponse & { data: Track[] }>(
    {
      url: '/playmode/intelligence/list',
      method: 'get',
      params,
    },
    decodeTrackDataResponse
  );
}
