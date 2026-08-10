import { describe, expect, test } from 'bun:test';
import {
  DEFAULT_API_PORT,
  DEFAULT_PROXY_RELAY_PORT,
  DEFAULT_WEB_PORT,
  parseSidecarArgs,
} from '../src/utils/sidecarConfig';

describe('sidecar 参数', () => {
  test('默认 UI 端口与 27232 兼容 API 隔离', () => {
    expect(DEFAULT_WEB_PORT).toBe(28_232);
  });

  test('API-only 模式使用默认 API 端口', () => {
    expect(parseSidecarArgs(['--api-only'])).toEqual({
      apiPort: DEFAULT_API_PORT,
      webPort: DEFAULT_WEB_PORT,
      rendererDir: null,
      apiOnly: true,
      proxyRelayPort: DEFAULT_PROXY_RELAY_PORT,
      upstreamProxy: null,
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
      proxyRelayPort: DEFAULT_PROXY_RELAY_PORT,
      upstreamProxy: null,
    });
  });

  test('拒绝无效端口，避免意外绑定随机端口', () => {
    expect(() => parseSidecarArgs(['--api-only', '--api-port', '0'])).toThrow(
      '--api-port 必须是 1 到 65535 之间的整数'
    );
  });

  test('UI 模式必须显式给出渲染目录', () => {
    expect(() => parseSidecarArgs([])).toThrow(
      '非 API-only 模式必须提供 --renderer-dir'
    );
  });

  test('代理 relay 参数必须成对且 upstream 只能是纯 HTTP(S) endpoint', () => {
    expect(
      parseSidecarArgs([
        '--api-only',
        '--proxy-relay-port',
        '28233',
        '--upstream-proxy',
        'http://proxy.example:8080',
      ])
    ).toMatchObject({
      proxyRelayPort: 28233,
      upstreamProxy: 'http://proxy.example:8080',
    });
    expect(
      parseSidecarArgs([
        '--api-only',
        '--upstream-proxy',
        'https://proxy.example:8443',
      ])
    ).toMatchObject({ upstreamProxy: 'https://proxy.example:8443' });
    expect(() =>
      parseSidecarArgs(['--api-only', '--proxy-relay-port', '28233'])
    ).toThrow('--proxy-relay-port 必须与 --upstream-proxy 一起使用');
    expect(() =>
      parseSidecarArgs([
        '--api-only',
        '--upstream-proxy',
        'https://user:pass@proxy.example:8080/path',
      ])
    ).toThrow('upstream proxy must contain only an HTTP(S) host and port');
  });
});
