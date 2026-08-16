import { getAppStore } from '@/stores/accessor';
import { fetchNeteaseLyrics } from '@/services/playbackSource';
import request from '@/utils/request';
import { mapTrackPlayableStatus } from '@/utils/common';
import {
  cacheTrackDetail,
  getTrackDetailFromCache,
  cacheLyric,
  getLyricFromCache,
} from '@/utils/db';
import type { Track } from '@/types/domain';
import type { LyricsResponse } from '@/utils/lyrics';
import type { ApiResponse, TrackCollectionResponse } from './types';
import {
  decodeApiResponse,
  decodeArray,
  decodeNumber,
  decodeOptionalNumber,
  decodeOptionalString,
  decodeRecord,
  decodeString,
  decodeTrack,
  decodeTrackCollectionResponse,
} from './decoders';
import type { DecodeContext, Decoder, ValueDecoder } from './decoders';

function decodeLyricPayload(
  input: unknown,
  context: DecodeContext,
  field: string
): { lyric?: string } {
  const payload = decodeRecord(input, context, field);
  const lyric = decodeOptionalString(
    payload['lyric'],
    context,
    `${field}.lyric`
  );
  return { ...payload, ...(lyric === undefined ? {} : { lyric }) };
}

const decodeCloudLyricResponse: Decoder<{
  lrc?: string | { lyric?: string };
}> = (input, context) => {
  const response = decodeRecord(input, context);
  if (response['lrc'] === undefined) return {};
  return {
    lrc:
      typeof response['lrc'] === 'string'
        ? decodeString(response['lrc'], context, '$.lrc')
        : decodeLyricPayload(response['lrc'], context, '$.lrc'),
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

export function getTrackDetail(
  ids: number | string
): Promise<TrackCollectionResponse> {
  const fetchLatest = () => {
    return request<TrackCollectionResponse>(
      {
        url: '/song/detail',
        method: 'get',
        params: {
          ids,
        },
      },
      decodeTrackCollectionResponse
    ).then(data => {
      data.songs.forEach(song => {
        const privileges = data.privileges?.find(t => t.id === song.id);
        cacheTrackDetail(song, privileges);
      });
      data.songs = mapTrackPlayableStatus(data.songs, data.privileges);
      return data;
    });
  };
  fetchLatest();

  let idsInArray = [String(ids)];
  if (typeof ids === 'string') {
    idsInArray = ids.split(',');
  }

  return (
    getTrackDetailFromCache(idsInArray) as Promise<
      TrackCollectionResponse | undefined
    >
  ).then(result => {
    if (result) {
      result.songs = mapTrackPlayableStatus(result.songs, result.privileges);
    }
    return result ?? fetchLatest();
  });
}

export function getLyric(id: number): Promise<LyricsResponse> {
  const fetchLatest = () => {
    // Typed sidecar endpoint (core::ncm lyric_new), shared with the TUI.
    return fetchNeteaseLyrics(id).then(result => {
      cacheLyric(id, result);
      return result;
    });
  };

  // Background refresh only; failures surface on the foreground path below.
  fetchLatest().catch(() => {});

  return (getLyricFromCache(id) as Promise<LyricsResponse | undefined>).then(
    result => {
      return result ?? fetchLatest();
    }
  );
}

export function getCloudLyric(
  songId: number,
  userId: number
): Promise<LyricsResponse> {
  const fetchLatest = () => {
    return request<{ lrc?: string | { lyric?: string } }>(
      {
        url: '/api',
        method: 'get',
        params: {
          uri: `/api/cloud/lyric/get`,
          data: {
            songId,
            userId,
            lv: '-1',
            kv: '-1',
          },
          crypto: 'eapi',
        },
      },
      decodeCloudLyricResponse
    ).then(result => {
      const lrc =
        typeof result.lrc === 'string' ? { lyric: result.lrc } : result.lrc;
      const normalized: LyricsResponse = lrc === undefined ? {} : { lrc };
      cacheLyric(songId, normalized);
      return normalized;
    });
  };

  fetchLatest();

  return getLyricFromCache(songId).then(result => {
    return result ?? fetchLatest();
  });
}

export function topSong(
  type: number
): Promise<ApiResponse & { data: Track[] }> {
  return request<ApiResponse & { data: Track[] }>(
    {
      url: '/top/song',
      method: 'get',
      params: {
        type,
      },
    },
    decodeTrackDataResponse
  );
}

export function likeATrack(params: {
  id: number;
  like?: boolean;
  timestamp?: number;
}): Promise<ApiResponse> {
  params.timestamp = new Date().getTime();
  return request<ApiResponse>(
    {
      url: '/like',
      method: 'get',
      params,
    },
    decodeApiResponse
  );
}

export function scrobble(params: {
  id: number;
  sourceid: number | string;
  time?: number;
  timestamp?: number;
}): Promise<ApiResponse> {
  params.timestamp = new Date().getTime();
  return request<ApiResponse>(
    {
      url: '/scrobble',
      method: 'get',
      params,
    },
    decodeApiResponse
  );
}
