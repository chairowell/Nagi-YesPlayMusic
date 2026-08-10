/**
 * Shared artwork rules prevent mixed-content failures in WKWebView.
 */

/**
 * Centralized sizes keep prefetch cache keys aligned with rendered images.
 */
export const ARTWORK_SIZE = Object.freeze({
  tray: 64, // The desktop process fetches the menu bar icon.
  miniPlayer: 128,
  playerBar: 224,
  coverColor: 256, // Used only for dominant-color extraction.
  lyricsBackground: 512,
  lyricsCover: 1024,
});

/**
 * Prefetch only small images to avoid competing with lossless audio downloads.
 */
export const PREFETCHED_ARTWORK_SIZES = Object.freeze([
  ARTWORK_SIZE.miniPlayer,
  ARTWORK_SIZE.playerBar,
  ARTWORK_SIZE.lyricsBackground,
]);

/**
 * Warm several covers so rapid track changes still hit the image cache.
 */
export const UPCOMING_ARTWORK_COUNT = 3;
export function buildArtworkURL(rawURL: unknown, size = 512): string {
  if (typeof rawURL !== 'string' || rawURL.trim() === '') return '';

  try {
    const url = new URL(rawURL.trim().replace(/^http:/, 'https:'));
    url.searchParams.set('param', `${size}y${size}`);
    return url.toString();
  } catch {
    return '';
  }
}
