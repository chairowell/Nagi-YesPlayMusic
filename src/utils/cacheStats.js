/**
 * 逐条统计音频缓存，避免把所有 ArrayBuffer 同时留在一个数组里。
 * iterate 接收一个访问器，因此可直接适配 Dexie Collection.each。
 */
export async function sumTrackSourceStats(iterate) {
  let bytes = 0;
  let length = 0;

  await iterate(track => {
    bytes += track?.source?.byteLength || 0;
    length += 1;
  });

  return { bytes, length };
}

/**
 * URL.revokeObjectURL 必须收到真实 URL；for...in 遍历数组只会得到下标字符串。
 */
export function revokeBlobURLs(urls, revoke) {
  for (const url of urls) revoke(url);
}

/**
 * 同一个 key 的并发任务共享一个 Promise，完成后再允许下一次执行。
 * 音频缓存借此避免同一首歌被同时下载、重复累计大小。
 */
export function createKeyedTaskPool() {
  const pending = new Map();

  return function runOnce(key, task) {
    if (pending.has(key)) return pending.get(key);

    const promise = Promise.resolve()
      .then(task)
      .finally(() => pending.delete(key));
    pending.set(key, promise);
    return promise;
  };
}
