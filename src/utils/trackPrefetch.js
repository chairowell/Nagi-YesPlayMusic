function artworkURLs(track) {
  const rawURL = track?.al?.picUrl ?? track?.album?.picUrl;
  if (typeof rawURL !== 'string' || rawURL.trim() === '') return [];

  try {
    const baseURL = new URL(rawURL.trim().replace(/^http:/, 'https:'));
    return [224, 512].map(size => {
      const sizedURL = new URL(baseURL);
      sizedURL.searchParams.set('param', `${size}y${size}`);
      return sizedURL.toString();
    });
  } catch {
    return [];
  }
}

/**
 * 只把封面交给 WebKit 的图片缓存，不保存 Image 引用，也不主动解码整张大图。
 */
export function warmTrackArtwork(track, createImage = () => new Image()) {
  const urls = artworkURLs(track);
  urls.forEach(src => {
    const image = createImage();
    image.decoding = 'async';
    image.src = src;
  });
  return urls;
}

/**
 * 预取永远只有一个逻辑目标；队列改变后，旧详情即使晚到也不能继续拉歌词或音频。
 */
export function createNextTrackPrefetcher({
  loadTrack,
  loadLyric,
  warmArtwork,
  cacheAudio,
}) {
  let generation = 0;
  let targetKey = null;
  let currentTask = null;

  const clear = () => {
    generation += 1;
    targetKey = null;
    currentTask = null;
  };

  const prefetch = (trackID, { includeAudio = false } = {}) => {
    if (!trackID) {
      clear();
      return Promise.resolve(null);
    }

    const nextKey = `${trackID}:${includeAudio ? 'audio' : 'basic'}`;
    if (nextKey === targetKey && currentTask) return currentTask;

    targetKey = nextKey;
    const requestGeneration = ++generation;
    const isCurrent = () =>
      requestGeneration === generation && targetKey === nextKey;

    currentTask = Promise.resolve()
      .then(() => loadTrack(trackID))
      .then(async track => {
        if (!track || !isCurrent()) return null;

        const tasks = [
          Promise.resolve().then(() => loadLyric(trackID)),
          Promise.resolve().then(() => warmArtwork(track)),
        ];
        if (includeAudio) {
          tasks.push(
            Promise.resolve().then(() => cacheAudio(track, isCurrent))
          );
        }
        await Promise.allSettled(tasks);
        return isCurrent() ? track : null;
      })
      .catch(error => {
        if (isCurrent()) {
          console.debug('[Player] 下一首预取失败，切歌时按正常路径加载', error);
        }
        return null;
      });

    return currentTask;
  };

  return { clear, prefetch };
}
