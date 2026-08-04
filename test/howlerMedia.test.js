import { describe, expect, test } from 'bun:test';
import { getHowlerMediaNode } from '../src/utils/howlerMedia';

describe('Howler 媒体节点适配层', () => {
  test('HTML5 模式返回原生媒体节点', () => {
    const node = {
      currentTime: 12,
      addEventListener() {},
      removeEventListener() {},
    };
    expect(getHowlerMediaNode({ _sounds: [{ _node: node }] })).toBe(node);
  });

  test('Web Audio 模式的 GainNode 不能被当成媒体节点', () => {
    // GainNode 也有 addEventListener（EventTarget），但没有 currentTime；
    // 把它当媒体节点会让 setSinkId 之类的调用直接抛错。
    const gainNode = {
      gain: {},
      addEventListener() {},
      removeEventListener() {},
    };
    expect(getHowlerMediaNode({ _sounds: [{ _node: gainNode }] })).toBeNull();
  });

  test('空实例与缺失节点都安全返回 null', () => {
    expect(getHowlerMediaNode(null)).toBeNull();
    expect(getHowlerMediaNode({ _sounds: [] })).toBeNull();
  });
});
