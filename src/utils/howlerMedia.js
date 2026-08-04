/**
 * Howler 没有公开 HTMLMediaElement，但原生 seeked 与输出设备都只能从这里拿。
 * 私有结构集中在这个适配层，避免播放器其他逻辑各自绑定 Howler 内部实现。
 */
export function getHowlerMediaNode(howler) {
  const node = howler?._sounds?.[0]?._node;
  return node && typeof node.addEventListener === 'function' ? node : null;
}
