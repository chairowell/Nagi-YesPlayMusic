import request from '@/utils/request';
import { mapTrackPlayableStatus } from '@/utils/common';
import { cacheAlbum, getAlbumFromCache } from '@/utils/db';
import type { Album, Artist } from '@/types/domain';
import type { TrackCollectionResponse, ApiResponse } from './types';
import {
  decodeAlbum,
  decodeArray,
  decodeArtist,
  decodeBoolean,
  decodeCodeResponse,
  decodeRecord,
  decodeTrackCollectionResponse,
} from './decoders';
import type { Decoder, ValueDecoder } from './decoders';

interface NewAlbumsResponse extends ApiResponse {
  albums: Album[];
}

export interface DetailedAlbum extends Album {
  artist: Artist;
}

interface AlbumResponse extends TrackCollectionResponse {
  album: DetailedAlbum;
}

interface AlbumDynamicResponse extends ApiResponse {
  isSub: boolean;
}

const decodeDetailedAlbum: ValueDecoder<DetailedAlbum> = (
  input,
  context,
  field
) => {
  const album = decodeAlbum(input, context, field);
  return {
    ...album,
    artist: decodeArtist(album['artist'], context, `${field}.artist`),
  };
};

const decodeAlbumResponse: Decoder<AlbumResponse> = (input, context) => {
  const response = decodeTrackCollectionResponse(input, context);
  return {
    ...response,
    album: decodeDetailedAlbum(response['album'], context, '$.album'),
  };
};

const decodeNewAlbumsResponse: Decoder<NewAlbumsResponse> = (
  input,
  context
) => {
  const response = decodeRecord(input, context);
  return {
    ...response,
    albums: decodeArray(response['albums'], context, '$.albums', decodeAlbum),
  };
};

const decodeAlbumDynamicResponse: Decoder<AlbumDynamicResponse> = (
  input,
  context
) => {
  const response = decodeRecord(input, context);
  return {
    ...response,
    isSub: decodeBoolean(response['isSub'], context, '$.isSub'),
  };
};

export function getAlbum(id: number): Promise<AlbumResponse> {
  const fetchLatest = () => {
    return request<AlbumResponse>(
      {
        url: '/album',
        method: 'get',
        params: {
          id,
        },
      },
      decodeAlbumResponse
    ).then(data => {
      cacheAlbum(id, data);
      data.songs = mapTrackPlayableStatus(data.songs);
      return data;
    });
  };
  fetchLatest();

  return (getAlbumFromCache(id) as Promise<AlbumResponse | undefined>).then(
    result => {
      return result ?? fetchLatest();
    }
  );
}

export function newAlbums(params: {
  limit: number;
  offset?: number;
  area: string;
}): Promise<NewAlbumsResponse> {
  return request<NewAlbumsResponse>(
    {
      url: '/album/new',
      method: 'get',
      params,
    },
    decodeNewAlbumsResponse
  );
}

export function albumDynamicDetail(id: number): Promise<AlbumDynamicResponse> {
  return request<AlbumDynamicResponse>(
    {
      url: '/album/detail/dynamic',
      method: 'get',
      params: { id, timestamp: new Date().getTime() },
    },
    decodeAlbumDynamicResponse
  );
}

export function likeAAlbum(params: {
  id: number;
  t: number;
}): Promise<ApiResponse & { code: number }> {
  return request<ApiResponse & { code: number }>(
    {
      url: '/album/sub',
      method: 'post',
      params,
    },
    decodeCodeResponse
  );
}
