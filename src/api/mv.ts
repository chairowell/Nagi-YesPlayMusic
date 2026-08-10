import request from '@/utils/request';
import type { ApiResponse } from './types';
import type { MusicVideo } from '@/types/domain';
import {
  decodeArray,
  decodeBoolean,
  decodeCodeResponse,
  decodeMusicVideo,
  decodeNumber,
  decodeOptionalString,
  decodeRecord,
  decodeString,
} from './decoders';
import type { Decoder, ValueDecoder } from './decoders';

export interface MvDetailData extends MusicVideo {
  artistId: number;
  artistName: string;
  playCount: number;
  publishTime: string;
  cover: string;
  brs: Array<{ br: number }>;
}

export interface MvDetailResponse extends ApiResponse {
  data: MvDetailData;
  subed: boolean;
}

interface MvUrlResponse extends ApiResponse {
  data: {
    id: number;
    url?: string;
    r: number;
  };
}

const decodeBitrate: ValueDecoder<{ br: number }> = (input, context, field) => {
  const bitrate = decodeRecord(input, context, field);
  return { br: decodeNumber(bitrate['br'], context, `${field}.br`) };
};

const decodeMvDetailData: ValueDecoder<MvDetailData> = (
  input,
  context,
  field
) => {
  const video = decodeMusicVideo(input, context, field);
  return {
    ...video,
    artistId: decodeNumber(video['artistId'], context, `${field}.artistId`),
    artistName: decodeString(
      video['artistName'],
      context,
      `${field}.artistName`
    ),
    playCount: decodeNumber(video['playCount'], context, `${field}.playCount`),
    publishTime: decodeString(
      video['publishTime'],
      context,
      `${field}.publishTime`
    ),
    cover: decodeString(video['cover'], context, `${field}.cover`),
    brs: decodeArray(video['brs'], context, `${field}.brs`, decodeBitrate),
  };
};

const decodeMvDetailResponse: Decoder<MvDetailResponse> = (input, context) => {
  const response = decodeRecord(input, context);
  return {
    ...response,
    data: decodeMvDetailData(response['data'], context, '$.data'),
    subed: decodeBoolean(response['subed'], context, '$.subed'),
  };
};

const decodeMvUrlResponse: Decoder<MvUrlResponse> = (input, context) => {
  const response = decodeRecord(input, context);
  const data = decodeRecord(response['data'], context, '$.data');
  const url = decodeOptionalString(data['url'], context, '$.data.url');
  return {
    ...response,
    data: {
      ...data,
      id: decodeNumber(data['id'], context, '$.data.id'),
      r: decodeNumber(data['r'], context, '$.data.r'),
      ...(url === undefined ? {} : { url }),
    },
  };
};

const decodeMvsResponse: Decoder<ApiResponse & { mvs: MusicVideo[] }> = (
  input,
  context
) => {
  const response = decodeRecord(input, context);
  return {
    ...response,
    mvs: decodeArray(response['mvs'], context, '$.mvs', decodeMusicVideo),
  };
};

export function mvDetail(mvid: number): Promise<MvDetailResponse> {
  return request<MvDetailResponse>(
    {
      url: '/mv/detail',
      method: 'get',
      params: {
        mvid,
        timestamp: new Date().getTime(),
      },
    },
    decodeMvDetailResponse
  );
}

export function mvUrl(params: {
  id: number;
  r?: number;
}): Promise<MvUrlResponse> {
  return request<MvUrlResponse>(
    {
      url: '/mv/url',
      method: 'get',
      params,
    },
    decodeMvUrlResponse
  );
}

export function simiMv(
  mvid: number
): Promise<ApiResponse & { mvs: MusicVideo[] }> {
  return request<ApiResponse & { mvs: MusicVideo[] }>(
    {
      url: '/simi/mv',
      method: 'get',
      params: { mvid },
    },
    decodeMvsResponse
  );
}

export function likeAMV(params: {
  mvid: number;
  t?: number;
  timestamp?: number;
}): Promise<ApiResponse & { code: number }> {
  params.timestamp = new Date().getTime();
  return request<ApiResponse & { code: number }>(
    {
      url: '/mv/sub',
      method: 'post',
      params,
    },
    decodeCodeResponse
  );
}
