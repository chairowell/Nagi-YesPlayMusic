import request from '@/utils/request';
import type { Track } from '@/types/domain';
import type { ApiResponse } from './types';
import { decodeApiResponse, decodeArray, decodeRecord, decodeTrack } from './decoders';
import type { Decoder } from './decoders';

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
