import { describe, expect, test } from 'bun:test';
import {
  DEFAULT_API_PORT,
  DEFAULT_WEB_PORT,
  parseSidecarArgs,
} from '../src/utils/sidecarConfig';

describe('sidecar 参数', () => {
  test('API-only 模式保留 Electron 使用的默认 API 端口', () => {
    expect(parseSidecarArgs(['--api-only'])).toEqual({
      apiPort: DEFAULT_API_PORT,
      webPort: DEFAULT_WEB_PORT,
      rendererDir: null,
      apiOnly: true,
    });
  });

  test('隔离测试可以覆盖端口和渲染目录', () => {
    expect(
      parseSidecarArgs([
        '--api-port',
        '11754',
        '--web-port',
        '28232',
        '--renderer-dir',
        '/tmp/renderer',
      ])
    ).toEqual({
      apiPort: 11754,
      webPort: 28232,
      rendererDir: '/tmp/renderer',
      apiOnly: false,
    });
  });

  test('拒绝无效端口，避免意外绑定随机端口', () => {
    expect(() =>
      parseSidecarArgs(['--api-only', '--api-port', '0'])
    ).toThrow('--api-port 必须是 1 到 65535 之间的整数');
  });

  test('UI 模式必须显式给出渲染目录', () => {
    expect(() => parseSidecarArgs([])).toThrow(
      '非 API-only 模式必须提供 --renderer-dir'
    );
  });
});
