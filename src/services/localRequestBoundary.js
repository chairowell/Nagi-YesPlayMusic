import { timingSafeEqual } from 'node:crypto';

export const NATIVE_AUTH_HEADER = 'x-yesplaymusic-native-token';

export function addNativeProxyToken(options, request, nativeToken) {
  if (request.originalUrl.startsWith('/api/native/')) {
    options.headers ??= {};
    options.headers[NATIVE_AUTH_HEADER] = nativeToken;
  }
  return options;
}

function hardenAuthCookie(cookie) {
  const [name] = cookie.split('=', 1);
  if (!['MUSIC_U', '__csrf'].includes(name.trim())) return cookie;
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

export function hardenAuthCookieHeaders(headers) {
  const key = Object.keys(headers).find(
    header => header.toLowerCase() === 'set-cookie'
  );
  if (!key || !headers[key]) return headers;
  const cookies = Array.isArray(headers[key]) ? headers[key] : [headers[key]];
  return {
    ...headers,
    [key]: cookies.map(hardenAuthCookie),
  };
}

function secretsMatch(received, expected) {
  if (typeof received !== 'string' || typeof expected !== 'string') {
    return false;
  }
  const receivedBytes = Buffer.from(received);
  const expectedBytes = Buffer.from(expected);
  return (
    receivedBytes.length === expectedBytes.length &&
    timingSafeEqual(receivedBytes, expectedBytes)
  );
}

function appendVaryHeader(response, name) {
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

function sendJson(response, statusCode, payload) {
  response.statusCode = statusCode;
  response.setHeader('Content-Type', 'application/json; charset=utf-8');
  response.setHeader('Cache-Control', 'no-store');
  response.end(JSON.stringify(payload));
}

export function installLocalRequestBoundary(
  app,
  { allowedOrigins, nativeToken = null }
) {
  const allowed = new Set(allowedOrigins);
  const boundary = (request, response, next) => {
    if (typeof request.originalUrl === 'string') {
      // 上游会把 originalUrl 原样写进 stdout；路由仍使用 req.url，所以只脱敏日志视图。
      request.originalUrl = request.originalUrl.split('?')[0];
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
      request.url.split('?')[0].startsWith('/native/') &&
      !secretsMatch(request.headers[NATIVE_AUTH_HEADER], nativeToken)
    ) {
      sendJson(response, 401, { message: 'native 接口认证失败' });
      return;
    }

    next();
  };

  app.use(boundary);
  // 网易云 API 返回 Express app 时业务路由已经全部注册，只能把刚加入的边界层
  // 移到 stack 最前面，确保恶意请求不会先触发有副作用的上游路由。
  const stack = app._router?.stack;
  if (!stack?.length) throw new Error('Express 路由栈不可用');
  stack.unshift(stack.pop());
  return boundary;
}
