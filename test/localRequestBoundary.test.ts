import { describe, expect, test } from 'bun:test';
import express from 'express';
import type { Application, Request, RequestHandler, Response } from 'express';
import type { IncomingHttpHeaders, OutgoingHttpHeaders } from 'node:http';
import {
  NATIVE_AUTH_HEADER,
  addNativeProxyToken,
  hardenAuthCookieHeaders,
  installLocalRequestBoundary,
} from '../src/services/localRequestBoundary';
import { desktopSessionExpiryCookies } from '../src/services/sidecarIdentity';
import { installDesktopLogoutRoute } from '../src/services/sidecarSession';
import { performLogout } from '../src/utils/logout';

interface TestLayer {
  handle: RequestHandler;
}

interface TestRouter {
  stack: TestLayer[];
}

type TestApplication = Application & { _router: TestRouter };

function createGuardedBoundary() {
  const app = express();
  const testApp = app as TestApplication;
  const existingRoute = () => {};
  app.get('/account', existingRoute);
  const originalFirstLayer = testApp._router.stack[0];

  // The boundary must precede routes registered by the upstream API factory.
  installLocalRequestBoundary(app, {
    allowedOrigins: ['http://127.0.0.1:28232'],
    nativeToken: 'test-native-token',
  });

  const boundaryLayer = testApp._router.stack[0];
  if (!boundaryLayer) throw new Error('Expected boundary layer');
  expect(boundaryLayer).not.toBe(originalFirstLayer);
  return (
    headers: IncomingHttpHeaders = {},
    url = '/account',
    method = 'GET'
  ) => {
    let nextCalled = false;
    const requestObject = { headers, url, originalUrl: url, method };
    const responseHeaders = new Map<string, unknown>();
    const response = Object.assign(
      Object.create(express.response) as Response,
      {
        statusCode: 200,
        body: null as unknown,
        setHeader(key: string, value: unknown) {
          responseHeaders.set(key.toLowerCase(), value);
        },
        getHeader(key: string) {
          return responseHeaders.get(key.toLowerCase());
        },
        end(body?: unknown) {
          this.body = body;
        },
      }
    );
    boundaryLayer.handle(
      requestObject as Request,
      response,
      () => (nextCalled = true)
    );
    return { requestObject, response, responseHeaders, nextCalled };
  };
}

describe('本地 HTTP 安全边界', () => {
  test('未知 Origin 在进入网易云路由前就被拒绝', () => {
    const request = createGuardedBoundary();
    const { response, nextCalled } = request({
      origin: 'https://evil.example',
    });

    expect(response.statusCode).toBe(403);
    expect(nextCalled).toBe(false);
  });

  test('只允许正式版页面读取响应并携带 cookie', () => {
    const request = createGuardedBoundary();
    const { response, responseHeaders, nextCalled } = request({
      origin: 'http://127.0.0.1:28232',
    });

    expect(response.statusCode).toBe(200);
    expect(nextCalled).toBe(true);
    expect(responseHeaders.get('access-control-allow-origin')).toBe(
      'http://127.0.0.1:28232'
    );
    expect(responseHeaders.get('access-control-allow-credentials')).toBe(
      'true'
    );
  });

  test('没有 Origin 但明确标记为跨站的浏览器请求也会被拒绝', () => {
    const request = createGuardedBoundary();
    const { response, nextCalled } = request({
      'sec-fetch-site': 'cross-site',
    });

    expect(response.statusCode).toBe(403);
    expect(nextCalled).toBe(false);
  });

  test('native 接口还必须携带 sidecar 内部令牌', () => {
    const request = createGuardedBoundary();
    const withoutToken = request(
      { origin: 'http://127.0.0.1:28232' },
      '/native/action'
    );
    const withToken = request(
      {
        origin: 'http://127.0.0.1:28232',
        [NATIVE_AUTH_HEADER]: 'test-native-token',
      },
      '/native/action'
    );

    expect(withoutToken.response.statusCode).toBe(401);
    expect(withoutToken.nextCalled).toBe(false);
    expect(withToken.response.statusCode).toBe(200);
    expect(withToken.nextCalled).toBe(true);
  });

  test('本机注销响应可靠清除 HttpOnly 会话', async () => {
    const cookies = desktopSessionExpiryCookies();
    expect(cookies).toEqual([
      expect.stringContaining('MUSIC_U=;'),
      expect.stringContaining('__csrf=;'),
    ]);
    for (const cookie of cookies) {
      expect(cookie).toContain('Max-Age=0');
      expect(cookie).toContain('HttpOnly');
      expect(cookie).toContain('SameSite=Strict');
    }
    let routePath: string | undefined;
    let routeHandler: RequestHandler | undefined;
    const requests: Array<{
      url: string;
      options: Parameters<typeof fetch>[1];
    }> = [];
    const logoutApp = {
      post: ((path: string, handler: RequestHandler) => {
        routePath = path;
        routeHandler = handler;
        return logoutApp;
      }) as Application['post'],
    };
    const requestLogout = Object.assign(
      async (
        url: Parameters<typeof fetch>[0],
        options?: Parameters<typeof fetch>[1]
      ) => {
        requests.push({ url: String(url), options });
        return new Response(null, { status: 204 });
      },
      { preconnect: fetch.preconnect }
    );
    installDesktopLogoutRoute(logoutApp, 12754, requestLogout);
    const responseHeaders = new Map<string, unknown>();
    let ended = false;
    const response = Object.assign(
      Object.create(express.response) as Response,
      {
        statusCode: 200,
        setHeader(name: string, value: unknown) {
          responseHeaders.set(name, value);
        },
        end() {
          ended = true;
        },
      }
    );

    if (!routeHandler) throw new Error('Expected logout route handler');
    routeHandler(
      { headers: { cookie: 'MUSIC_U=secret' } } as Request,
      response,
      () => {}
    );
    await Promise.resolve();

    expect(routePath).toBe('/native/logout-session');
    expect(requests).toEqual([
      {
        url: 'http://127.0.0.1:12754/logout',
        options: {
          method: 'POST',
          headers: { Cookie: 'MUSIC_U=secret' },
        },
      },
    ]);
    expect(response.statusCode).toBe(204);
    expect(responseHeaders.get('Set-Cookie')).toEqual(cookies);
    expect(responseHeaders.get('Cache-Control')).toBe('no-store');
    expect(ended).toBe(true);
  });

  test('桌面注销必须等待本机 HttpOnly 会话清除后再清理界面状态', async () => {
    const events: Array<string | [string, unknown]> = [];
    const result = await performLogout(
      { clearUserSession: () => events.push('clear-local-state') },
      {
        isTauri: true,
        clearDesktopSession: async () => events.push('clear-http-only-cookie'),
        requestWebLogout: () => events.push('web-logout'),
        removeWebCookie: key => events.push(['remove-cookie', key]),
        reportError: error => events.push(['error', error]),
      }
    );

    expect(result).toBe(true);
    expect(events).toEqual(['clear-http-only-cookie', 'clear-local-state']);
  });

  test('只有内部 native 代理会注入令牌', () => {
    const nativeHeaders: OutgoingHttpHeaders = {};
    const normalHeaders: OutgoingHttpHeaders = {};
    const nativeOptions = addNativeProxyToken(
      { headers: nativeHeaders },
      { originalUrl: '/api/native/unblock-music' },
      'proxy-token'
    );
    const normalOptions = addNativeProxyToken(
      { headers: normalHeaders },
      { originalUrl: '/api/login/status' },
      'proxy-token'
    );

    expect(nativeOptions.headers[NATIVE_AUTH_HEADER]).toBe('proxy-token');
    expect(normalOptions.headers[NATIVE_AUTH_HEADER]).toBeUndefined();
  });

  test('业务路由保留查询参数，但日志使用的 originalUrl 会去掉隐私参数', () => {
    const request = createGuardedBoundary();
    const { requestObject, nextCalled } = request(
      {},
      '/login?email=user@example.com&md5_password=secret'
    );

    expect(nextCalled).toBe(true);
    expect(requestObject.url).toContain('md5_password=secret');
    expect(requestObject.originalUrl).toBe('/login');
  });

  test('代理把网易云登录 Cookie 强制升级为 HttpOnly 和 SameSite=Strict', () => {
    const headers = hardenAuthCookieHeaders({
      'set-cookie': [
        'MUSIC_U=secret; Path=/; SameSite=None',
        '__csrf=csrf; Path=/',
        'NMTID=ordinary; Path=/',
      ],
    });
    const cookies = headers['set-cookie'];
    if (!Array.isArray(cookies) || cookies.length !== 3) {
      throw new Error('Expected three Set-Cookie headers');
    }
    const [musicCookie, csrfCookie, ordinaryCookie] = cookies;
    if (!musicCookie || !csrfCookie || !ordinaryCookie) {
      throw new Error('Expected non-empty Set-Cookie headers');
    }

    expect(musicCookie).toContain('HttpOnly');
    expect(musicCookie).toContain('SameSite=Strict');
    expect(musicCookie).not.toContain('SameSite=None');
    expect(csrfCookie).toContain('HttpOnly');
    expect(ordinaryCookie).toBe('NMTID=ordinary; Path=/');
  });
});
