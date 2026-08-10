interface ApiCacheController {
  options(options: { enabled: boolean }): void;
}

interface AudioResponse extends Record<string, unknown> {
  id?: unknown;
}

export function configureSafeNeteaseApiCache(
  apiCache: ApiCacheController
): void {
  // The upstream cache ignores query and body parameters in this integration.
  apiCache.options({ enabled: false });
}

export function findMatchingAudioResponse<TResponse extends AudioResponse>(
  responses: readonly TResponse[],
  trackID: number | string
): TResponse | null;
export function findMatchingAudioResponse(
  responses: unknown,
  trackID: number | string
): AudioResponse | null;
export function findMatchingAudioResponse(
  responses: unknown,
  trackID: number | string
): AudioResponse | null {
  if (!Array.isArray(responses)) return null;
  const expectedID = Number(trackID);
  return (
    responses.find(
      (response): response is AudioResponse =>
        typeof response === 'object' &&
        response !== null &&
        'id' in response &&
        Number(response.id) === expectedID
    ) || null
  );
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
