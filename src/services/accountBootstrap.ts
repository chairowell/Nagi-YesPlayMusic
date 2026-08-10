interface AccountBootstrapOperations {
  accountLoggedIn: boolean;
  fetchUserProfile(): Promise<boolean>;
  fetchLikedSongs(): Promise<void>;
  fetchLikedPlaylist(): Promise<void>;
  fetchLikedSongsWithDetails(): Promise<void>;
}

export async function refreshAccountData({
  accountLoggedIn,
  fetchUserProfile,
  fetchLikedSongs,
  fetchLikedPlaylist,
  fetchLikedSongsWithDetails,
}: AccountBootstrapOperations): Promise<boolean> {
  if (!accountLoggedIn || !(await fetchUserProfile())) return false;
  await Promise.all([fetchLikedSongs(), fetchLikedPlaylist()]);
  await fetchLikedSongsWithDetails();
  return true;
}
