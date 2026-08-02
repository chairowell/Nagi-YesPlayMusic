import { describe, expect, test } from 'bun:test';
import express from 'express';
import {
  NATIVE_AUTH_HEADER,
  addNativeProxyToken,
  installLocalRequestBoundary,
} from '../src/services/localRequestBoundary';

function createGuardedBoundary() {
  const app = express();
  const existingRoute = () => {};
  app.get('/account', existingRoute);
  const originalFirstLayer = app._router.stack[0];

  // 上游网易云 API 在返回 app 时已经注册完路由，安全边界必须能插到已有路由之前。
  installLocalRequestBoundary(app, {
    allowedOrigins: ['http://127.0.0.1:28232'],
    nativeToken: 'test-native-token',
  });

  const boundaryLayer = app._router.stack[0];
  expect(boundaryLayer).not.toBe(originalFirstLayer);
  return (headers = {}, url = '/account') => {
    let nextCalled = false;
    const responseHeaders = new Map();
    const response = {
      statusCode: 200,
      body: null,
      status(code) {
        this.statusCode = code;
        return this;
      },
      send(body) {
        this.body = body;
        return this;
      },
      vary(value) {
        responseHeaders.set('vary', value);
        return this;
      },
      set(values) {
        for (const [key, value] of Object.entries(values)) {
          responseHeaders.set(key.toLowerCase(), value);
        }
        return this;
      },
    };
    boundaryLayer.handle(
      { headers, url },
      response,
      () => (nextCalled = true)
    );
    return { response, responseHeaders, nextCalled };
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

  test('只有内部 native 代理会注入令牌', () => {
    const nativeOptions = addNativeProxyToken(
      { headers: {} },
      { originalUrl: '/api/native/unblock-music' },
      'proxy-token'
    );
    const normalOptions = addNativeProxyToken(
      { headers: {} },
      { originalUrl: '/api/login/status' },
      'proxy-token'
    );

    expect(nativeOptions.headers[NATIVE_AUTH_HEADER]).toBe('proxy-token');
    expect(normalOptions.headers[NATIVE_AUTH_HEADER]).toBeUndefined();
  });
});
