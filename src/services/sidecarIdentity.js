import { createInterface } from 'node:readline';

export const SIDECAR_HEALTH_PATH = '/__yesplaymusic/health';
export const SIDECAR_HEALTH_BODY = JSON.stringify({
  service: 'yesplaymusic-sidecar',
  protocol: 1,
});
export const SIDECAR_HEALTH_TOKEN_HEADER = 'X-YesPlayMusic-Health-Token';

function requireSidecarHealthToken(token) {
  if (!/^[0-9a-f]{64}$/.test(token || '')) {
    throw new Error('sidecar 启动令牌缺失或格式不合法');
  }
  return token;
}

export async function readSidecarHealthToken(input = process.stdin) {
  const reader = createInterface({ input, crlfDelay: Infinity });
  try {
    const { value, done } = await reader[Symbol.asyncIterator]().next();
    if (done) throw new Error('父进程未提供 sidecar 启动令牌');
    return requireSidecarHealthToken(value);
  } finally {
    reader.close();
    // 父进程会继续保留管道以管理进程；读完首行后不应让 stdin 阻止正常退出。
    input.pause?.();
  }
}

export function desktopSessionExpiryCookies() {
  const attributes = [
    'Path=/',
    'Max-Age=0',
    'Expires=Thu, 01 Jan 1970 00:00:00 GMT',
    'HttpOnly',
    'SameSite=Strict',
  ].join('; ');
  return [`MUSIC_U=; ${attributes}`, `__csrf=; ${attributes}`];
}

export function installSidecarHealthRoute(app, healthToken) {
  const token = requireSidecarHealthToken(healthToken);
  app.get(SIDECAR_HEALTH_PATH, (_request, response) => {
    // Bun 单文件编译后不会可靠保留 Express response 的便捷方法，使用 Node 原生接口。
    response.statusCode = 200;
    response.setHeader('Content-Type', 'application/json; charset=utf-8');
    response.setHeader('Cache-Control', 'no-store');
    response.setHeader(SIDECAR_HEALTH_TOKEN_HEADER, token);
    response.end(SIDECAR_HEALTH_BODY);
  });

  // 网易云 API 在返回 app 前已装好全部路由；健康检查必须排在业务路由前面。
  const healthLayer = app._router.stack.pop();
  app._router.stack.unshift(healthLayer);
}
