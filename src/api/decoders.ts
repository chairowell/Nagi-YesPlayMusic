import type {
  Album,
  Artist,
  MusicVideo,
  Playlist,
  Track,
  TrackPrivilege,
  UnknownRecord,
  UserProfile,
} from '@/types/domain';
import type { ApiResponse, TrackCollectionResponse } from './types';

export interface DecodeContext {
  url: string;
}

export type Decoder<T> = (input: unknown, context: DecodeContext) => T;
export type ValueDecoder<T> = (
  input: unknown,
  context: DecodeContext,
  field: string
) => T;

export class ApiContractError extends Error {
  readonly url: string;
  readonly field: string;

  constructor(url: string, field: string, expected: string) {
    super(`API 响应契约错误：${url} 的 ${field} 应为${expected}`);
    this.name = 'ApiContractError';
    this.url = url;
    this.field = field;
  }
}

function contractError(
  context: DecodeContext,
  field: string,
  expected: string
): never {
  throw new ApiContractError(context.url, field, expected);
}

export function decodeRecord(
  input: unknown,
  context: DecodeContext,
  field = '$'
): UnknownRecord {
  if (typeof input !== 'object' || input === null || Array.isArray(input)) {
    return contractError(context, field, '对象');
  }
  return { ...input };
}

export function decodeNumber(
  input: unknown,
  context: DecodeContext,
  field: string
): number {
  if (typeof input !== 'number' || !Number.isFinite(input)) {
    return contractError(context, field, '有限数字');
  }
  return input;
}

export function decodeString(
  input: unknown,
  context: DecodeContext,
  field: string
): string {
  if (typeof input !== 'string') {
    return contractError(context, field, '字符串');
  }
  return input;
}

export function decodeBoolean(
  input: unknown,
  context: DecodeContext,
  field: string
): boolean {
  if (typeof input !== 'boolean') {
    return contractError(context, field, '布尔值');
  }
  return input;
}

export function decodeOptionalNumber(
  input: unknown,
  context: DecodeContext,
  field: string
): number | undefined {
  return input === undefined ? undefined : decodeNumber(input, context, field);
}

export function decodeOptionalString(
  input: unknown,
  context: DecodeContext,
  field: string
): string | undefined {
  return input === undefined ? undefined : decodeString(input, context, field);
}

export function decodeOptionalBoolean(
  input: unknown,
  context: DecodeContext,
  field: string
): boolean | undefined {
  return input === undefined ? undefined : decodeBoolean(input, context, field);
}

export function decodeArray<T>(
  input: unknown,
  context: DecodeContext,
  field: string,
  itemDecoder: ValueDecoder<T>
): T[] {
  if (!Array.isArray(input)) {
    return contractError(context, field, '数组');
  }
  return input.map((item, index) =>
    itemDecoder(item, context, `${field}[${index}]`)
  );
}

export function decodeOptionalArray<T>(
  input: unknown,
  context: DecodeContext,
  field: string,
  itemDecoder: ValueDecoder<T>
): T[] | undefined {
  return input === undefined
    ? undefined
    : decodeArray(input, context, field, itemDecoder);
}

export const decodeTrack: ValueDecoder<Track> = (input, context, field) => {
  const track = decodeRecord(input, context, field);
  return {
    ...track,
    id: decodeNumber(track['id'], context, `${field}.id`),
  };
};

export const decodeTrackPrivilege: ValueDecoder<TrackPrivilege> = (
  input,
  context,
  field
) => {
  const privilege = decodeRecord(input, context, field);
  const id = decodeOptionalNumber(privilege['id'], context, `${field}.id`);
  const pl = decodeOptionalNumber(privilege['pl'], context, `${field}.pl`);
  const cs = decodeOptionalBoolean(privilege['cs'], context, `${field}.cs`);
  const fee = decodeOptionalNumber(privilege['fee'], context, `${field}.fee`);
  const st = decodeOptionalNumber(privilege['st'], context, `${field}.st`);
  return {
    ...privilege,
    ...(id === undefined ? {} : { id }),
    ...(pl === undefined ? {} : { pl }),
    ...(cs === undefined ? {} : { cs }),
    ...(fee === undefined ? {} : { fee }),
    ...(st === undefined ? {} : { st }),
  };
};

export const decodeArtist: ValueDecoder<Artist> = (input, context, field) => {
  const artist = decodeRecord(input, context, field);
  return {
    ...artist,
    id: decodeNumber(artist['id'], context, `${field}.id`),
  };
};

export const decodeAlbum: ValueDecoder<Album> = (input, context, field) => {
  const album = decodeRecord(input, context, field);
  return {
    ...album,
    id: decodeNumber(album['id'], context, `${field}.id`),
  };
};

export const decodePlaylist: ValueDecoder<Playlist> = (
  input,
  context,
  field
) => {
  const playlist = decodeRecord(input, context, field);
  return {
    ...playlist,
    id: decodeNumber(playlist['id'], context, `${field}.id`),
  };
};

export const decodeMusicVideo: ValueDecoder<MusicVideo> = (
  input,
  context,
  field
) => {
  const video = decodeRecord(input, context, field);
  return {
    ...video,
    id: decodeNumber(video['id'], context, `${field}.id`),
  };
};

export const decodeUserProfile: ValueDecoder<UserProfile> = (
  input,
  context,
  field
) => {
  const profile = decodeRecord(input, context, field);
  const userId = decodeOptionalNumber(
    profile['userId'],
    context,
    `${field}.userId`
  );
  const nickname = decodeOptionalString(
    profile['nickname'],
    context,
    `${field}.nickname`
  );
  const avatarUrl = decodeOptionalString(
    profile['avatarUrl'],
    context,
    `${field}.avatarUrl`
  );
  const vipType = decodeOptionalNumber(
    profile['vipType'],
    context,
    `${field}.vipType`
  );
  const signature = decodeOptionalString(
    profile['signature'],
    context,
    `${field}.signature`
  );
  return {
    ...profile,
    ...(userId === undefined ? {} : { userId }),
    ...(nickname === undefined ? {} : { nickname }),
    ...(avatarUrl === undefined ? {} : { avatarUrl }),
    ...(vipType === undefined ? {} : { vipType }),
    ...(signature === undefined ? {} : { signature }),
  };
};

export const decodeApiResponse: Decoder<ApiResponse> = (input, context) =>
  decodeRecord(input, context);

export const decodeCodeResponse: Decoder<ApiResponse & { code: number }> = (
  input,
  context
) => {
  const response = decodeRecord(input, context);
  return {
    ...response,
    code: decodeNumber(response['code'], context, '$.code'),
  };
};

export const decodeTrackCollectionResponse: Decoder<TrackCollectionResponse> = (
  input: unknown,
  context: DecodeContext
) => {
  const response = decodeRecord(input, context);
  const privileges = decodeOptionalArray(
    response['privileges'],
    context,
    '$.privileges',
    decodeTrackPrivilege
  );
  const album =
    response['album'] === undefined
      ? undefined
      : decodeAlbum(response['album'], context, '$.album');
  return {
    ...response,
    songs: decodeArray(response['songs'], context, '$.songs', decodeTrack),
    ...(privileges === undefined ? {} : { privileges }),
    ...(album === undefined ? {} : { album }),
  };
};
