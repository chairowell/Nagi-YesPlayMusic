import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import {
  CONTENT_SECURITY_POLICY,
  applyRendererSecurityHeaders,
} from '../src/services/contentSecurityPolicy';
import type { Request, Response } from 'express';

const tauriConfig = JSON.parse(
  readFileSync(new URL('../src-tauri/tauri.conf.json', import.meta.url), 'utf8')
);

describe('渲染进程内容安全策略', () => {
  test('实际 HTTP 响应与 Tauri 配置使用同一份策略', () => {
    expect(tauriConfig.app.security.csp).toBe(CONTENT_SECURITY_POLICY);

    const headers = new Map<string, string>();
    let nextCalled = false;
    const request = Object.create(null) as Request;
    const response = Object.assign(Object.create(null) as Response, {
      setHeader(name: string, value: string) {
        headers.set(name.toLowerCase(), value);
        return response;
      },
    });
    applyRendererSecurityHeaders(request, response, () => (nextCalled = true));

    expect(headers.get('content-security-policy')).toBe(
      CONTENT_SECURITY_POLICY
    );
    expect(headers.get('x-content-type-options')).toBe('nosniff');
    expect(headers.get('referrer-policy')).toBe('no-referrer');
    expect(nextCalled).toBe(true);
  });

  test('禁止远程脚本、内联脚本、对象嵌入和页面套壳', () => {
    expect(CONTENT_SECURITY_POLICY).toContain("script-src 'self'");
    expect(CONTENT_SECURITY_POLICY).not.toContain("script-src 'unsafe-inline'");
    expect(CONTENT_SECURITY_POLICY).not.toContain("script-src 'unsafe-eval'");
    expect(CONTENT_SECURITY_POLICY).toContain("object-src 'none'");
    expect(CONTENT_SECURITY_POLICY).toContain("frame-ancestors 'none'");
  });
});
