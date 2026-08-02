import { describe, expect, test } from 'bun:test';
import { hasReachableWindowArea } from '../src/utils/windowGeometry';

const displays = [
  { x: 0, y: 0, width: 2560, height: 1410 },
  // 混合 DPI 下 Tauri 保存的是 backing 坐标：第二块 Retina 屏从 5120 开始。
  { x: 5120, y: 0, width: 3024, height: 1900 },
];

describe('窗口可见边界', () => {
  test('拒绝本次真实出现的屏幕外坐标', () => {
    expect(
      hasReachableWindowArea(
        { x: 8064, y: 100, width: 3812, height: 268 },
        displays
      )
    ).toBe(false);
  });

  test('正常窗口和跨屏窗口仍可恢复', () => {
    expect(
      hasReachableWindowArea(
        { x: 837, y: 30, width: 920, height: 620 },
        displays
      )
    ).toBe(true);
    expect(
      hasReachableWindowArea(
        { x: 5000, y: 100, width: 600, height: 100 },
        displays
      )
    ).toBe(true);
  });

  test('只剩几个像素贴在屏幕边缘不算可操作', () => {
    expect(
      hasReachableWindowArea(
        { x: 8080, y: 100, width: 500, height: 200 },
        displays
      )
    ).toBe(false);
  });
});
