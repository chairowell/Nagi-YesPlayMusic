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
