import request from '@/utils/request';
import { mapTrackPlayableStatus } from '@/utils/common';
import type {
  Album,
  Artist,
  MusicVideo,
  Playlist,
  Track,
  UserProfile,
} from '@/types/domain';
import type { ApiResponse } from './types';
import {
  decodeAlbum,
  decodeApiResponse,
  decodeArray,
  decodeArtist,
  decodeBoolean,
  decodeMusicVideo,
  decodeNumber,
  decodeOptionalArray,
  decodeOptionalNumber,
  decodePlaylist,
  decodeRecord,
  decodeTrack,
  decodeUserProfile,
} from './decoders';
import type { Decoder, ValueDecoder } from './decoders';

export interface SearchResult {
  songs?: Track[];
  mvs?: MusicVideo[];
  albums?: Album[];
  artists?: Artist[];
  playlists?: Playlist[];
  userprofiles?: UserProfile[];
  mvCount?: number;
  albumCount?: number;
  hasMore?: boolean;
  song?: { songs: Track[] };
  [key: string]: unknown;
}

interface SearchResponse extends ApiResponse {
  result?: SearchResult;
}

const decodeSearchResult: ValueDecoder<SearchResult> = (
  input,
  context,
  field
) => {
  const result = decodeRecord(input, context, field);
  const songs = decodeOptionalArray(
    result['songs'],
    context,
    `${field}.songs`,
    decodeTrack
  );
  const mvs = decodeOptionalArray(
    result['mvs'],
    context,
    `${field}.mvs`,
    decodeMusicVideo
  );
  const albums = decodeOptionalArray(
    result['albums'],
    context,
    `${field}.albums`,
    decodeAlbum
  );
  const artists = decodeOptionalArray(
    result['artists'],
    context,
    `${field}.artists`,
    decodeArtist
  );
  const playlists = decodeOptionalArray(
    result['playlists'],
    context,
    `${field}.playlists`,
    decodePlaylist
  );
  const userprofiles = decodeOptionalArray(
    result['userprofiles'],
    context,
    `${field}.userprofiles`,
    decodeUserProfile
  );
  const mvCount = decodeOptionalNumber(
    result['mvCount'],
    context,
    `${field}.mvCount`
  );
  const albumCount = decodeOptionalNumber(
    result['albumCount'],
    context,
    `${field}.albumCount`
  );
  const hasMore =
    result['hasMore'] === undefined
      ? undefined
      : decodeBoolean(result['hasMore'], context, `${field}.hasMore`);
  const songRecord =
    result['song'] === undefined
      ? undefined
      : decodeRecord(result['song'], context, `${field}.song`);
  const song =
    songRecord === undefined
      ? undefined
      : {
          ...songRecord,
          songs: decodeArray(
            songRecord['songs'],
            context,
            `${field}.song.songs`,
            decodeTrack
          ),
        };
  return {
    ...result,
    ...(songs === undefined ? {} : { songs }),
    ...(mvs === undefined ? {} : { mvs }),
    ...(albums === undefined ? {} : { albums }),
    ...(artists === undefined ? {} : { artists }),
    ...(playlists === undefined ? {} : { playlists }),
    ...(userprofiles === undefined ? {} : { userprofiles }),
    ...(mvCount === undefined ? {} : { mvCount }),
    ...(albumCount === undefined ? {} : { albumCount }),
    ...(hasMore === undefined ? {} : { hasMore }),
    ...(song === undefined ? {} : { song }),
  };
};

const decodeSearchResponse: Decoder<SearchResponse> = (input, context) => {
  const response = decodeRecord(input, context);
  const result =
    response['result'] === undefined
      ? undefined
      : decodeSearchResult(response['result'], context, '$.result');
  return { ...response, ...(result === undefined ? {} : { result }) };
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

export function search(params: {
  keywords: string;
  limit?: number;
  offset?: number;
  type?: number;
}): Promise<SearchResponse> {
  return request<SearchResponse>(
    {
      url: '/search',
      method: 'get',
      params,
    },
    decodeSearchResponse
  ).then(data => {
    if (data.result?.songs !== undefined) {
      data.result.songs = mapTrackPlayableStatus(data.result.songs);
    }
    if (data.result?.song !== undefined) {
      data.result.song.songs = mapTrackPlayableStatus(data.result.song.songs);
    }
    return data;
  });
}

export function personalFM(): Promise<ApiResponse & { data: Track[] }> {
  return request<ApiResponse & { data: Track[] }>(
    {
      url: '/personal_fm',
      method: 'get',
      params: {
        timestamp: new Date().getTime(),
      },
    },
    decodeTrackDataResponse
  );
}

export function fmTrash(id: number): Promise<ApiResponse> {
  return request<ApiResponse>(
    {
      url: '/fm_trash',
      method: 'post',
      params: {
        timestamp: new Date().getTime(),
        id,
      },
    },
    decodeApiResponse
  );
}
