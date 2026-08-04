/**
 * 所有界面共用同一套封面规则，避免 WKWebView 因某个调用点漏转 HTTPS 而显示破图。
 */

/**
 * 正在播放的界面会用到的封面尺寸，集中在这里定义。
 *
 * 尺寸就是缓存键的一部分：预取 224，界面却要 128，等于预取了一张没人要的图，
 * 真正要显示的那张仍然得现下。迷你播放条以前正是这样（预取 224/512、自己要
 * 1024），所以切歌时封面总要等 0.6~1.5 秒。改任何一个尺寸都必须同步这里。
 */
export const ARTWORK_SIZE = Object.freeze({
  tray: 64, // 菜单栏图标，主进程自己取
  miniPlayer: 128,
  playerBar: 224,
  coverColor: 256, // 只用来算主色，不上屏
  lyricsBackground: 512,
  lyricsCover: 1024,
});

/**
 * 预取下一首时提前塞进图片缓存的尺寸。只放小图：命中缓存的封面 1~4ms 就出图，
 * 而无损音频本来就在抢带宽，不值得为大图再多占几十 KB。
 */
export const PREFETCHED_ARTWORK_SIZES = Object.freeze([
  ARTWORK_SIZE.miniPlayer,
  ARTWORK_SIZE.playerBar,
  ARTWORK_SIZE.lyricsBackground,
]);
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
