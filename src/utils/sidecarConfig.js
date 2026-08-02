const DEFAULT_API_PORT = 10754;
const DEFAULT_WEB_PORT = 27232;

function parsePort(value, flag) {
  const port = Number(value);
  if (!Number.isInteger(port) || port < 1 || port > 65535) {
    throw new Error(`${flag} 必须是 1 到 65535 之间的整数`);
  }
  return port;
}

function readValue(args, index, flag) {
  const value = args[index + 1];
  if (!value || value.startsWith('--')) {
    throw new Error(`${flag} 缺少参数`);
  }
  return value;
}

export function parseSidecarArgs(args) {
  const config = {
    apiPort: DEFAULT_API_PORT,
    webPort: DEFAULT_WEB_PORT,
    rendererDir: null,
    apiOnly: false,
  };

  for (let index = 0; index < args.length; index += 1) {
    const flag = args[index];
    switch (flag) {
      case '--api-port': {
        const value = readValue(args, index, flag);
        config.apiPort = parsePort(value, flag);
        index += 1;
        break;
      }
      case '--web-port': {
        const value = readValue(args, index, flag);
        config.webPort = parsePort(value, flag);
        index += 1;
        break;
      }
      case '--renderer-dir':
        config.rendererDir = readValue(args, index, flag);
        index += 1;
        break;
      case '--api-only':
        config.apiOnly = true;
        break;
      default:
        throw new Error(`未知参数：${flag}`);
    }
  }

  if (!config.apiOnly && !config.rendererDir) {
    throw new Error('非 API-only 模式必须提供 --renderer-dir');
  }

  return config;
}

export { DEFAULT_API_PORT, DEFAULT_WEB_PORT };
