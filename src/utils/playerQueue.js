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
/**
 * 往后数 count 首的 id，用来提前预热封面。
 *
 * 只预取下一首救不了"第一次见的封面"：实测切歌后封面仍要等 0.75~1.6 秒，
 * 因为预取排在音源解析（song/url 实测 1.9 秒）后面，还没跑完人就切走了。
 * 队列到头就停；已经取过的 id 不重复（单曲循环、超短队列会绕回自己）。
 */
export function getUpcomingTrackIDs(
  list,
  current,
  direction,
  shouldWrap,
  count
) {
  const ids = [];
  // 正在播的这首不用预热（它自己正在加载），绕回它就说明队列已经走完一圈
  const seen = new Set([list[current]]);
  let index = current;
  for (let step = 0; step < count; step += 1) {
    const [id, next] = getAdjacentTrack(list, index, direction, shouldWrap);
    if (id === undefined || seen.has(id)) break;
    seen.add(id);
    ids.push(id);
    index = next;
  }
  return ids;
}

export function getAdjacentTrack(list, current, direction, shouldWrap) {
  if (list.length === 0) return [undefined, current];

  let target = current + direction;
  if (target < 0 || target >= list.length) {
    if (!shouldWrap) return [undefined, target];
    target = target < 0 ? list.length - 1 : 0;
  }

  return [list[target], target];
}
