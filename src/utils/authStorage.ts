interface AccountSessionInput {
  isDesktop: boolean;
  loginMode: string | null;
  readableCookie: string | undefined;
}

interface StorageLike {
  readonly length: number;
  key(index: number): string | null;
  removeItem(key: string): void;
}

export function shouldUseLegacyCookieFallback(isDesktop: boolean): boolean {
  return !isDesktop;
}

export function hasAccountSession({
  isDesktop,
  loginMode,
  readableCookie,
}: AccountSessionInput): boolean {
  return loginMode === 'account' && (isDesktop || readableCookie !== undefined);
}

export function purgeLegacyDesktopAuthStorage(
  storage: StorageLike,
  isDesktop: boolean
): number {
  if (!isDesktop) return 0;
  const keys = Array.from({ length: storage.length }, (_, index) =>
    storage.key(index)
  ).filter((key): key is string => key !== null && key.startsWith('cookie-'));
  for (const key of keys) storage.removeItem(key);
  return keys.length;
}
