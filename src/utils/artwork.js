/**
 * 所有界面共用同一套封面规则，避免 WKWebView 因某个调用点漏转 HTTPS 而显示破图。
 */
export function buildArtworkURL(rawURL, size = 512) {
  if (typeof rawURL !== 'string' || rawURL.trim() === '') return '';

  try {
    const url = new URL(rawURL.trim().replace(/^http:/, 'https:'));
    url.searchParams.set('param', `${size}y${size}`);
    return url.toString();
  } catch {
    return '';
  }
}
