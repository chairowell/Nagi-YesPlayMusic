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

/**
 * 返回队列中指定方向的相邻歌曲。
 *
 * 正序、倒序只是方向不同，首尾循环应该共用同一套边界判断；分别写条件时，
 * “上一首”很容易把第一首和最后一首判断反。
 */
export function getAdjacentTrack(list, current, direction, shouldWrap) {
  if (list.length === 0) return [undefined, current];

  let target = current + direction;
  if (target < 0 || target >= list.length) {
    if (!shouldWrap) return [undefined, target];
    target = target < 0 ? list.length - 1 : 0;
  }

  return [list[target], target];
}
