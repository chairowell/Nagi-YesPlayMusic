interface ApiCacheController {
  options(options: { enabled: boolean }): void;
}

export function configureSafeNeteaseApiCache(
  apiCache: ApiCacheController
): void {
  // The upstream cache ignores query and body parameters in this integration.
  apiCache.options({ enabled: false });
}

export function isTrustedTrackSource(
  record: unknown,
  requestedTrackID: number | string
): boolean {
  if (typeof record !== 'object' || record === null) return false;
  const source = record as Record<string, unknown>;
  const expectedID = Number(requestedTrackID);
  return (
    Number(source['id']) === expectedID &&
    Number(source['validatedTrackID']) === expectedID
  );
}
