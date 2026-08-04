/**
 * Howler 没有公开 HTMLMediaElement，但原生 seeked 与输出设备都只能从这里拿。
 * 私有结构集中在这个适配层，避免播放器其他逻辑各自绑定 Howler 内部实现。
 */
export function getHowlerMediaNode(howler) {
  const node = howler?._sounds?.[0]?._node;
  // Web Audio 模式下 _node 是 GainNode，同样有 addEventListener（EventTarget），
  // 用 currentTime 区分出真正的 HTMLMediaElement。
  return node &&
    typeof node.addEventListener === 'function' &&
    'currentTime' in node
    ? node
    : null;
}
