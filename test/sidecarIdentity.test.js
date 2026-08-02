import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import express from 'express';
import {
  SIDECAR_HEALTH_BODY,
  SIDECAR_HEALTH_PATH,
  installSidecarHealthRoute,
} from '../src/services/sidecarIdentity';

describe('sidecar 身份握手', () => {
  const healthToken = 'a'.repeat(64);

  test('健康检查始终排在已有业务路由之前并返回本次启动的身份', () => {
    const app = express();
    app.get('/existing', () => {});
    installSidecarHealthRoute(app, healthToken);

    const healthLayer = app._router.stack[0];
    expect(healthLayer.route.path).toBe(SIDECAR_HEALTH_PATH);

    const headers = new Map();
    let body = null;
    healthLayer.route.stack[0].handle(
      {},
      {
        setHeader(name, value) {
          headers.set(name.toLowerCase(), value);
        },
        end(value) {
          body = value;
        },
      }
    );

    expect(headers.get('content-type')).toBe(
      'application/json; charset=utf-8'
    );
    expect(headers.get('x-yesplaymusic-health-token')).toBe(healthToken);
    expect(body).toBe(SIDECAR_HEALTH_BODY);
  });

  test('拒绝缺失或格式不合法的启动令牌', () => {
    const app = express();
    expect(() => installSidecarHealthRoute(app)).toThrow();
    expect(() => installSidecarHealthRoute(app, 'predictable')).toThrow();
  });

  test('Rust 启动器与 JavaScript sidecar 锁定同一份握手正文', () => {
    const rustSource = readFileSync(
      new URL('../src-tauri/src/main.rs', import.meta.url),
      'utf8'
    );
    expect(rustSource).toContain(
      `const SIDECAR_HEALTH_BODY: &str = r#"${SIDECAR_HEALTH_BODY}"#;`
    );
    expect(rustSource).toContain('child.write(');
    expect(rustSource).toContain(
      'response_has_sidecar_identity(response: &str, expected_token: &str)'
    );
  });
});
