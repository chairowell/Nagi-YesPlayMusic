import { parseUpstreamProxy } from '../services/webviewProxyRelay';

const DEFAULT_API_PORT = 10754;
const DEFAULT_WEB_PORT = 28232;
const DEFAULT_PROXY_RELAY_PORT = 27233;

export interface SidecarConfig {
  apiPort: number;
  webPort: number;
  rendererDir: string | null;
  apiOnly: boolean;
  proxyRelayPort: number;
  upstreamProxy: string | null;
  parentPid: number | null;
}

function parsePort(value: string, flag: string): number {
  const port = Number(value);
  if (!Number.isInteger(port) || port < 1 || port > 65535) {
    throw new Error(`${flag} 必须是 1 到 65535 之间的整数`);
  }
  return port;
}

function readValue(args: string[], index: number, flag: string): string {
  const value = args[index + 1];
  if (!value || value.startsWith('--')) {
    throw new Error(`${flag} 缺少参数`);
  }
  return value;
}

function parseParentPid(value: string): number {
  const pid = Number(value);
  if (!Number.isSafeInteger(pid) || pid < 1) {
    throw new Error('--parent-pid 必须是正整数');
  }
  return pid;
}

export function parseSidecarArgs(args: string[]): SidecarConfig {
  const config: SidecarConfig = {
    apiPort: DEFAULT_API_PORT,
    webPort: DEFAULT_WEB_PORT,
    rendererDir: null,
    apiOnly: false,
    proxyRelayPort: DEFAULT_PROXY_RELAY_PORT,
    upstreamProxy: null,
    parentPid: null,
  };

  for (let index = 0; index < args.length; index += 1) {
    const flag = args[index];
    switch (flag) {
      case '--api-port': {
        const value = readValue(args, index, flag ?? '--api-port');
        config.apiPort = parsePort(value, flag ?? '--api-port');
        index += 1;
        break;
      }
      case '--web-port': {
        const value = readValue(args, index, flag ?? '--web-port');
        config.webPort = parsePort(value, flag ?? '--web-port');
        index += 1;
        break;
      }
      case '--renderer-dir':
        config.rendererDir = readValue(args, index, flag ?? '--renderer-dir');
        index += 1;
        break;
      case '--api-only':
        config.apiOnly = true;
        break;
      case '--proxy-relay-port': {
        const value = readValue(args, index, flag ?? '--proxy-relay-port');
        config.proxyRelayPort = parsePort(value, flag ?? '--proxy-relay-port');
        index += 1;
        break;
      }
      case '--upstream-proxy':
        config.upstreamProxy = readValue(
          args,
          index,
          flag ?? '--upstream-proxy'
        );
        index += 1;
        break;
      case '--parent-pid':
        config.parentPid = parseParentPid(
          readValue(args, index, flag ?? '--parent-pid')
        );
        index += 1;
        break;
      default:
        throw new Error(`未知参数：${flag}`);
    }
  }

  if (!config.apiOnly && !config.rendererDir) {
    throw new Error('非 API-only 模式必须提供 --renderer-dir');
  }
  if (!config.upstreamProxy && args.includes('--proxy-relay-port')) {
    throw new Error('--proxy-relay-port 必须与 --upstream-proxy 一起使用');
  }
  if (config.upstreamProxy) parseUpstreamProxy(config.upstreamProxy);

  return config;
}

export { DEFAULT_API_PORT, DEFAULT_PROXY_RELAY_PORT, DEFAULT_WEB_PORT };
