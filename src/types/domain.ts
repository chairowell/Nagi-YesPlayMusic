export type UnknownRecord = Record<string, unknown>;

export interface UserProfile {
  userId?: number;
  nickname?: string;
  avatarUrl?: string;
  vipType?: number;
  signature?: string;
  [key: string]: unknown;
}

export interface Artist extends UnknownRecord {
  id: number;
  name?: string;
  picUrl?: string;
  img1v1Url?: string;
  followed?: boolean;
  albumSize?: number;
  musicSize?: number;
  mvSize?: number;
  briefDesc?: string;
}

export interface Album extends UnknownRecord {
  id: number;
  name?: string;
  picUrl?: string;
  type?: string;
  size?: number;
  publishTime?: number;
  artist?: Artist;
  artists?: Artist[];
  mark?: number;
  description?: string;
  company?: string;
}

export interface Track extends UnknownRecord {
  id: number;
  songId?: number;
  name?: string;
  dt?: number;
  ar?: Artist[];
  artists?: Artist[];
  al?: Album;
  album?: Album;
  playCount?: number;
  playable?: boolean;
  reason?: string;
  fee?: number;
  noCopyrightRcmd?: unknown;
  privilege?: TrackPrivilege;
  sort?: number;
  mark?: number;
  alia?: string[];
  tns?: string[];
  simpleSong?: Track;
  songName?: string;
  pc?: unknown | null;
  cd?: unknown | null;
}

export interface TrackPrivilege extends UnknownRecord {
  id?: number;
  pl?: number;
  cs?: boolean;
  fee?: number;
  st?: number;
}

export interface Playlist extends UnknownRecord {
  id: number;
  name?: string;
  coverImgUrl?: string;
  picUrl?: string;
  trackCount?: number;
  playCount?: number;
  subscribed?: boolean;
  creator?: UserProfile;
  trackIds?: Array<{ id: number }>;
  tracks?: Track[];
  privacy?: number;
  updateTime?: number;
  description?: string;
  englishTitle?: string;
  updateFrequency?: string;
}

export interface MusicVideo extends UnknownRecord {
  id: number;
  vid?: number;
  name?: string;
  title?: string;
  cover?: string;
  coverUrl?: string;
  imgurl16v9?: string;
  publishTime?: string;
  artistName?: string;
  artistId?: number;
  creator?: Array<{ userName?: string; userId?: number }>;
}

export interface LastfmState extends UnknownRecord {
  key?: string;
  name?: string;
}
