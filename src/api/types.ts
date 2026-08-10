import type { Album, Track, TrackPrivilege } from '@/types/domain';

export type ApiResponse = Record<string, unknown>;

export type { TrackPrivilege } from '@/types/domain';

export interface TrackCollectionResponse extends ApiResponse {
  songs: Track[];
  privileges?: TrackPrivilege[];
  album?: Album;
}
