interface FetchLikedSongsDependencies {
  isLooseLoggedIn: () => boolean;
  isAccountLoggedIn: () => boolean;
  fetchLikedSongIds: (userId: number) => Promise<{ ids?: number[] }>;
}

export async function fetchLikedSongIdsForUser(
  userId: number | undefined,
  dependencies: FetchLikedSongsDependencies
): Promise<number[] | null> {
  if (
    userId === undefined ||
    !dependencies.isLooseLoggedIn() ||
    !dependencies.isAccountLoggedIn()
  ) {
    return null;
  }

  const result = await dependencies.fetchLikedSongIds(userId);
  return result.ids ?? null;
}
