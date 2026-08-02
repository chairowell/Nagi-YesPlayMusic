import { timingSafeEqual } from 'node:crypto';

export const NATIVE_AUTH_HEADER = 'x-yesplaymusic-native-token';

export function addNativeProxyToken(options, request, nativeToken) {
  if (request.originalUrl.startsWith('/api/native/')) {
    options.headers ??= {};
    options.headers[NATIVE_AUTH_HEADER] = nativeToken;
  }
  return options;
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

export function installLocalRequestBoundary(
  app,
  { allowedOrigins, nativeToken = null }
) {
  const allowed = new Set(allowedOrigins);
  const boundary = (request, response, next) => {
    const origin = request.headers.origin;
    const fetchSite = request.headers['sec-fetch-site'];
    if (
      fetchSite === 'cross-site' ||
      (typeof origin === 'string' && !allowed.has(origin))
    ) {
      response.status(403).send({ message: '拒绝跨站访问本地服务' });
      return;
    }

    if (origin) {
      response.vary('Origin');
      response.set({
        'Access-Control-Allow-Origin': origin,
        'Access-Control-Allow-Credentials': 'true',
      });
    }

    if (
      nativeToken &&
      request.url.split('?')[0].startsWith('/native/') &&
      !secretsMatch(request.headers[NATIVE_AUTH_HEADER], nativeToken)
    ) {
      response.status(401).send({ message: 'native 接口认证失败' });
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
