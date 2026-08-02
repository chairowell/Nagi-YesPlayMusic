/**
 * 返回歌曲在当前实际播放队列中的位置。
 *
 * 随机模式维护的是独立队列，不能拿原始列表的下标去更新随机队列位置，
 * 否则手动点歌后“下一首”会从旧的随机位置继续。
 */
export function getActiveTrackIndex({ shuffle, list, shuffledList }, trackID) {
  const activeList = shuffle ? shuffledList : list;
  return activeList.findIndex(id => id === trackID);
}

/**
 * 从歌单中选一首作为心动模式种子。
 * 随机数只能乘列表长度；多加一会让最大下标越过数组末尾。
 */
export function pickRandomTrackID(trackIds, random = Math.random) {
  if (trackIds.length === 0) return undefined;
  const index = Math.floor(random() * trackIds.length);
  return trackIds[index]?.id;
}

/**
 * 用户直接播放插队歌曲时要消费掉这一项，否则下一次切歌会再次命中它。
 * 只移除第一项，保留用户有意把同一首歌插队多次的语义。
 */
export function consumeQueuedTrack(queue, trackID) {
  const index = queue.findIndex(id => id === trackID);
  if (index !== -1) queue.splice(index, 1);
  return index;
}
