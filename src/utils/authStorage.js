export function shouldUseLegacyCookieFallback(isDesktop) {
  return !isDesktop;
}

export function purgeLegacyDesktopAuthStorage(storage, isDesktop) {
  if (!isDesktop) return 0;
  const keys = Array.from(
    { length: storage.length },
    (_, index) => storage.key(index)
  ).filter(key => key?.startsWith('cookie-'));
  for (const key of keys) storage.removeItem(key);
  return keys.length;
}
