export function createTrackSwitchGuard() {
  let generation = 0;

  return Object.freeze({
    begin() {
      generation += 1;
      return generation;
    },
    isCurrent(candidate) {
      return candidate === generation;
    },
  });
}

/**
 * 歌曲详情和音源都要过同一个 generation 检查。
 * 只比较 track id 不够：详情尚未返回时，播放器甚至还不知道哪个请求已经过期。
 */
export async function runLatestTrackSwitch(
  guard,
  { onBegin, loadTrack, commitTrack, loadSource, commitSource, onMissingSource }
) {
  const generation = guard.begin();
  onBegin?.();

  const track = await loadTrack();
  if (!guard.isCurrent(generation)) return false;

  commitTrack(track);
  const source = await loadSource(track);
  if (!guard.isCurrent(generation)) return false;

  if (!source) {
    onMissingSource?.(track);
    return false;
  }

  commitSource(source, track);
  return true;
}
