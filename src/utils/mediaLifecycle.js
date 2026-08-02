/**
 * Plyr 会注册媒体事件并持有 video 节点，离开页面时必须显式销毁。
 */
export function destroyMediaPlayer(player) {
  player?.destroy?.();
}
