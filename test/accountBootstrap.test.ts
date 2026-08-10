import { describe, expect, test } from 'bun:test';
import { refreshAccountData } from '../src/services/accountBootstrap';

describe('account startup refresh', () => {
  test('refreshes the cookie owner before loading dependent library data', async () => {
    const sequence: string[] = [];
    let songsReady = false;
    let playlistReady = false;

    const refreshed = await refreshAccountData({
      accountLoggedIn: true,
      fetchUserProfile: async () => {
        sequence.push('profile');
        return true;
      },
      fetchLikedSongs: async () => {
        sequence.push('songs');
        songsReady = true;
      },
      fetchLikedPlaylist: async () => {
        sequence.push('playlist');
        playlistReady = true;
      },
      fetchLikedSongsWithDetails: async () => {
        expect(songsReady).toBeTrue();
        expect(playlistReady).toBeTrue();
        sequence.push('details');
      },
    });

    expect(refreshed).toBeTrue();
    expect(sequence).toEqual(['profile', 'songs', 'playlist', 'details']);
  });

  test('does not query another account when profile refresh fails', async () => {
    let dependentRequests = 0;
    const skipped = async () => {
      dependentRequests += 1;
    };

    const refreshed = await refreshAccountData({
      accountLoggedIn: true,
      fetchUserProfile: async () => false,
      fetchLikedSongs: skipped,
      fetchLikedPlaylist: skipped,
      fetchLikedSongsWithDetails: skipped,
    });

    expect(refreshed).toBeFalse();
    expect(dependentRequests).toBe(0);
  });
});
