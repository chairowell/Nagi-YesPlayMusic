import { timingSafeEqual } from 'node:crypto';
import type { Application, NextFunction, Request, Response } from 'express';
import type { OutgoingHttpHeaders } from 'node:http';

export const NATIVE_AUTH_HEADER = 'x-yesplaymusic-native-token';

interface ProxyRequestOptions {
  headers?: OutgoingHttpHeaders;
}

interface OriginalUrlRequest {
  originalUrl: string;
}

interface ExpressLayerStack {
  stack: unknown[];
}

type ExpressAppWithRouter = Application & {
  _router?: ExpressLayerStack;
};

export function addNativeProxyToken<T extends ProxyRequestOptions>(
  options: T,
  request: OriginalUrlRequest,
  nativeToken: string
): T {
  if (request.originalUrl.startsWith('/api/native/')) {
    options.headers ??= {};
    options.headers[NATIVE_AUTH_HEADER] = nativeToken;
  }
  return options;
}

function hardenAuthCookie(cookie: string): string {
  const [name] = cookie.split('=', 1);
  if (!name || !['MUSIC_U', '__csrf'].includes(name.trim())) return cookie;
  const parts = cookie
    .split(';')
    .map(part => part.trim())
    .filter(
      part =>
        part &&
        part.toLowerCase() !== 'httponly' &&
        !part.toLowerCase().startsWith('samesite=')
    );
  return [...parts, 'HttpOnly', 'SameSite=Strict'].join('; ');
}

export function hardenAuthCookieHeaders(
  headers: OutgoingHttpHeaders
): OutgoingHttpHeaders {
  const key = Object.keys(headers).find(
    header => header.toLowerCase() === 'set-cookie'
  );
  if (!key || !headers[key]) return headers;
  const rawCookies = headers[key];
  const cookies = Array.isArray(rawCookies) ? rawCookies : [String(rawCookies)];
  return {
    ...headers,
    [key]: cookies.map(hardenAuthCookie),
  };
}

function secretsMatch(received: unknown, expected: unknown): boolean {
  if (typeof received !== 'string' || typeof expected !== 'string') {
    return false;
  }
  const encoder = new TextEncoder();
  const receivedBytes = encoder.encode(received);
  const expectedBytes = encoder.encode(expected);
  return (
    receivedBytes.length === expectedBytes.length &&
    timingSafeEqual(receivedBytes, expectedBytes)
  );
}

function appendVaryHeader(response: Response, name: string): void {
  const current = response.getHeader('Vary');
  const values = (
    Array.isArray(current) ? current : String(current || '').split(',')
  )
    .map(value => value.trim())
    .filter(Boolean);
  if (!values.some(value => value.toLowerCase() === name.toLowerCase())) {
    values.push(name);
  }
  response.setHeader('Vary', values.join(', '));
}

function sendJson(
  response: Response,
  statusCode: number,
  payload: unknown
): void {
  response.statusCode = statusCode;
  response.setHeader('Content-Type', 'application/json; charset=utf-8');
  response.setHeader('Cache-Control', 'no-store');
  response.end(JSON.stringify(payload));
}

export function installLocalRequestBoundary(
  app: ExpressAppWithRouter,
  {
    allowedOrigins,
    nativeToken = null,
  }: { allowedOrigins: string[]; nativeToken?: string | null }
): (request: Request, response: Response, next: NextFunction) => void {
  const allowed = new Set(allowedOrigins);
  const boundary = (
    request: Request,
    response: Response,
    next: NextFunction
  ) => {
    if (typeof request.originalUrl === 'string') {
      // Redact the logged originalUrl while preserving req.url for routing.
      request.originalUrl = request.originalUrl.split('?')[0] ?? '';
    }
    const origin = request.headers.origin;
    const fetchSite = request.headers['sec-fetch-site'];
    if (
      fetchSite === 'cross-site' ||
      (typeof origin === 'string' && !allowed.has(origin))
    ) {
      sendJson(response, 403, { message: '拒绝跨站访问本地服务' });
      return;
    }

    if (origin) {
      appendVaryHeader(response, 'Origin');
      response.setHeader('Access-Control-Allow-Origin', origin);
      response.setHeader('Access-Control-Allow-Credentials', 'true');
    }

    if (
      nativeToken &&
      (request.url.split('?')[0] ?? '').startsWith('/native/') &&
      !secretsMatch(request.headers[NATIVE_AUTH_HEADER], nativeToken)
    ) {
      sendJson(response, 401, { message: 'native 接口认证失败' });
      return;
    }

    next();
  };

  app.use(boundary);
  // Move the late boundary layer first so unsafe requests never reach upstream routes.
  const stack = app._router?.stack;
  if (!stack?.length) throw new Error('Express 路由栈不可用');
  const boundaryLayer = stack.pop();
  if (boundaryLayer === undefined) throw new Error('Express 安全边界注册失败');
  stack.unshift(boundaryLayer);
  return boundary;
}
