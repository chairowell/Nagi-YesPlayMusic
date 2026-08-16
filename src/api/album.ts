import request from '@/utils/request';
import { mapTrackPlayableStatus } from '@/utils/common';
import { fetchAlbumDetail } from '@/services/detailSource';
import { cacheAlbum, getAlbumFromCache } from '@/utils/db';
import type { Album, Artist } from '@/types/domain';
import type { TrackCollectionResponse, ApiResponse } from './types';
import {
  decodeAlbum,
  decodeArray,
  decodeBoolean,
  decodeCodeResponse,
  decodeRecord,
} from './decoders';
import type { Decoder } from './decoders';

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
    // Typed sidecar endpoint (core::ncm), shared with the TUI.
    return fetchAlbumDetail(id).then((data: AlbumResponse) => {
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
