import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import express from 'express';
import {
  SIDECAR_HEALTH_BODY,
  SIDECAR_HEALTH_PATH,
  installSidecarHealthRoute,
} from '../src/services/sidecarIdentity';

describe('sidecar 身份握手', () => {
  test('健康检查始终排在已有业务路由之前并返回固定身份', () => {
    const app = express();
    app.get('/existing', () => {});
    installSidecarHealthRoute(app);

    const healthLayer = app._router.stack[0];
    expect(healthLayer.route.path).toBe(SIDECAR_HEALTH_PATH);

    let contentType = null;
    let body = null;
    healthLayer.route.stack[0].handle(
      {},
      {
        type(value) {
          contentType = value;
          return this;
        },
        send(value) {
          body = value;
          return this;
        },
      }
    );

    expect(contentType).toBe('application/json');
    expect(body).toBe(SIDECAR_HEALTH_BODY);
  });

  test('Rust 启动器与 JavaScript sidecar 锁定同一份握手正文', () => {
    const rustSource = readFileSync(
      new URL('../src-tauri/src/main.rs', import.meta.url),
      'utf8'
    );
    expect(rustSource).toContain(
      `const SIDECAR_HEALTH_BODY: &str = r#"${SIDECAR_HEALTH_BODY}"#;`
    );
  });
});
