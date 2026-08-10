import type Player from '@/utils/Player';
import type {
  Album,
  Artist,
  LastfmState,
  MusicVideo,
  Playlist,
  Track,
} from './domain';
import type { DataState, SettingsState } from './persistence';

export interface ToastState {
  show: boolean;
  text: string;
  timer: ReturnType<typeof setTimeout> | null;
}

export interface ModalsState {
  addTrackToPlaylistModal: {
    show: boolean;
    selectedTrackID: number;
  };
  newPlaylistModal: {
    show: boolean;
    afterCreateAddTrackID: number;
  };
}

export type ModalUpdate =
  | {
      modalName: 'addTrackToPlaylistModal';
      key: 'show';
      value: boolean;
    }
  | {
      modalName: 'addTrackToPlaylistModal';
      key: 'selectedTrackID';
      value: number;
    }
  | {
      modalName: 'newPlaylistModal';
      key: 'show';
      value: boolean;
    }
  | {
      modalName: 'newPlaylistModal';
      key: 'afterCreateAddTrackID';
      value: number;
    };

export interface PlayHistoryState {
  weekData: Track[];
  allData: Track[];
}

export interface LikedState {
  songs: number[];
  songsWithDetails: Track[];
  playlists: Playlist[];
  albums: Album[];
  artists: Artist[];
  mvs: MusicVideo[];
  cloudDisk: Track[];
  playHistory: PlayHistoryState;
}

export interface AppState {
  sessionEpoch: number;
  showLyrics: boolean;
  enableScrolling: boolean;
  title: string;
  liked: LikedState;
  contextMenu: {
    clickObjectID: number;
    showMenu: boolean;
  };
  toast: ToastState;
  modals: ModalsState;
  dailyTracks: Track[];
  lastfm: LastfmState;
  player: Player;
  settings: SettingsState;
  data: DataState;
}
