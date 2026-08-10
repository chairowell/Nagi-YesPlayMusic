import { createInterface } from 'node:readline';
import type { Application, Request, Response } from 'express';
import type { Readable } from 'node:stream';

export const SIDECAR_HEALTH_PATH = '/__yesplaymusic/health';
export const SIDECAR_HEALTH_BODY = JSON.stringify({
  service: 'yesplaymusic-sidecar',
  protocol: 1,
});
export const SIDECAR_HEALTH_TOKEN_HEADER = 'X-YesPlayMusic-Health-Token';

interface ExpressLayerStack {
  stack: unknown[];
}

type ExpressAppWithRouter = Application & {
  _router: ExpressLayerStack;
};

function requireSidecarHealthToken(token: unknown): string {
  if (typeof token !== 'string' || !/^[0-9a-f]{64}$/.test(token)) {
    throw new Error('sidecar 启动令牌缺失或格式不合法');
  }
  return token;
}

export async function readSidecarHealthToken(
  input: Readable = process.stdin
): Promise<string> {
  const reader = createInterface({ input, crlfDelay: Infinity });
  try {
    const { value, done } = await reader[Symbol.asyncIterator]().next();
    if (done) throw new Error('父进程未提供 sidecar 启动令牌');
    return requireSidecarHealthToken(value);
  } finally {
    reader.close();
    // Release stdin after the token so the parent pipe cannot delay shutdown.
    input.pause?.();
  }
}

export function desktopSessionExpiryCookies(): string[] {
  const attributes = [
    'Path=/',
    'Max-Age=0',
    'Expires=Thu, 01 Jan 1970 00:00:00 GMT',
    'HttpOnly',
    'SameSite=Strict',
  ].join('; ');
  return [`MUSIC_U=; ${attributes}`, `__csrf=; ${attributes}`];
}

export function installSidecarHealthRoute(
  app: ExpressAppWithRouter,
  healthToken: string
): void {
  const token = requireSidecarHealthToken(healthToken);
  app.get(SIDECAR_HEALTH_PATH, (_request: Request, response: Response) => {
    // Use Node primitives because Bun single-file builds may omit Express helpers.
    response.statusCode = 200;
    response.setHeader('Content-Type', 'application/json; charset=utf-8');
    response.setHeader('Cache-Control', 'no-store');
    response.setHeader(SIDECAR_HEALTH_TOKEN_HEADER, token);
    response.end(SIDECAR_HEALTH_BODY);
  });

  // Put health checks before the routes already installed by the upstream API.
  const healthLayer = app._router.stack.pop();
  if (healthLayer === undefined) {
    throw new Error('sidecar 健康检查路由注册失败');
  }
  app._router.stack.unshift(healthLayer);
}
