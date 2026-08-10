import { describe, expect, test } from 'bun:test';
import { detectPlatform } from '../src/utils/platform';

describe('跨运行环境的平台识别', () => {
  test('Node 环境优先使用 process.platform', () => {
    expect(detectPlatform({ platform: 'darwin' }, {})).toBe('darwin');
  });

  test('纯浏览器没有 process 时从 Navigator 识别 macOS', () => {
    expect(detectPlatform(null, { platform: 'MacIntel' })).toBe('darwin');
  });

  test('纯浏览器没有 process 时从 Navigator 识别 Windows', () => {
    expect(
      detectPlatform(null, { userAgentData: { platform: 'Windows' } })
    ).toBe('win32');
  });
});
