/**
 * FLAC 拖拽升级为精确 WAV 源的编排器。对播放器与网络的全部操作都经
 * 注入的依赖完成，竞态规则集中于此并被单元测试直接驱动：
 *
 * - 同一时刻只跑一个升级任务；连续拖拽合并为最新目标（drain 循环）。
 * - 每个请求携带发起时的 seek 代际；任务开始前代际已变化的请求直接丢弃。
 * - 任务结束时 howler / 歌曲 / 代际任何一项变化都放弃应用：切歌后的
 *   拖拽由下一轮以新快照重跑，期间的流式 seek（含跳到 0 秒）绝不被
 *   旧任务的目标覆盖回跳。
 * - 恢复播放同时看播放状态与"暂停手势还在 fade 中"的意图标记。
 */
export function createPreciseSeekUpgrader({
  getSnapshot,
  readCachedFlac,
  convertViaSidecar,
  convertInRenderer,
  freezeAt,
  seekStream,
  applyPreciseSource,
  onError = () => {},
}) {
  let pending = null;
  let draining = false;

  function request(time, sendMpris) {
    pending = {
      time: Math.max(0, Number(time) || 0),
      sendMpris,
      token: getSnapshot().seekToken,
    };
    if (!draining) void drain();
  }

  async function drain() {
    draining = true;
    try {
      while (pending) {
        const req = pending;
        pending = null;
        // 排队期间来了更新的 seek（任何路径），这个目标已经过时
        if (getSnapshot().seekToken !== req.token) continue;
        await runOnce(req);
      }
    } finally {
      draining = false;
    }
  }

  const changedSince = start => {
    const now = getSnapshot();
    return (
      now.howler !== start.howler ||
      now.trackId !== start.trackId ||
      now.seekToken !== start.seekToken
    );
  };

  async function runOnce(req) {
    const start = getSnapshot();
    // 转换期间冻结歌词时钟，进度条先显示用户请求的位置
    freezeAt(req.time);
    let url = null;
    try {
      const cached = await readCachedFlac(start.trackId);
      if (cached && !changedSince(start)) {
        url = await convertViaSidecar(
          start.trackId,
          cached.bytes,
          cached.bitsPerSample
        );
        if (!url && !changedSince(start)) {
          url = await convertInRenderer(cached.bytes);
        }
      }
    } catch (error) {
      onError(error);
      url = null;
    }

    const now = getSnapshot();
    if (now.howler !== start.howler || now.trackId !== start.trackId) {
      // 切歌/换源：新实例的替换清理已接管 _seeking，本次结果作废
      return;
    }
    if (now.seekToken !== start.seekToken) {
      // 期间用户又 seek 了：流式事务已自行生效，或新的升级请求在排队
      return;
    }
    if (pending) return; // 让循环用最新目标重跑
    if (!url) {
      // 缓存未写完或全部转换失败：解冻并按流式路径 seek
      seekStream(req.time, req.sendMpris);
      return;
    }
    applyPreciseSource(
      url,
      req.time,
      req.sendMpris,
      now.playing && !now.pausePending
    );
  }

  return {
    request,
    get busy() {
      return draining;
    },
  };
}
