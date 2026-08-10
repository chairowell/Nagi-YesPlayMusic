import { describe, expect, test } from 'bun:test';
import { PassThrough } from 'node:stream';
import { readFileSync } from 'node:fs';
import express from 'express';
import type { Request, Response } from 'express';
import {
  SIDECAR_HEALTH_BODY,
  SIDECAR_HEALTH_PATH,
  installSidecarHealthRoute,
  monitorParentProcess,
  monitorSidecarParent,
  readSidecarHealthToken,
} from '../src/services/sidecarIdentity';

interface TestRouteLayer {
  route: {
    path: string;
    stack: Array<{
      handle(request: Request, response: Response): void;
    }>;
  };
}

describe('sidecar 身份握手', () => {
  const healthToken = 'a'.repeat(64);

  test('健康检查始终排在已有业务路由之前并返回本次启动的身份', () => {
    const app = express();
    app.get('/existing', () => {});
    installSidecarHealthRoute(app, healthToken);

    const router = app._router as { stack: TestRouteLayer[] };
    const healthLayer = router.stack[0];
    if (!healthLayer) throw new Error('健康检查路由未注册');
    expect(healthLayer.route.path).toBe(SIDECAR_HEALTH_PATH);

    const headers = new Map<string, string | number | readonly string[]>();
    const body: { value: string | null } = { value: null };
    const request = Object.create(null) as Request;
    const response = Object.assign(Object.create(null) as Response, {
      setHeader(
        name: string,
        value: string | number | readonly string[]
      ): Response {
        headers.set(name.toLowerCase(), value);
        return response;
      },
      end(value: string): Response {
        body.value = value;
        return response;
      },
    });
    const handler = healthLayer.route.stack[0];
    if (!handler) throw new Error('健康检查处理器未注册');
    handler.handle(request, response);

    expect(headers.get('content-type')).toBe('application/json; charset=utf-8');
    expect(headers.get('x-yesplaymusic-health-token')).toBe(healthToken);
    expect(body.value).toBe(SIDECAR_HEALTH_BODY);
  });

  test('拒绝缺失或格式不合法的启动令牌', () => {
    const app = express();
    expect(() => installSidecarHealthRoute(app, '')).toThrow();
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
    expect(rustSource).toContain('"--parent-pid".to_string()');
  });

  test('API-only 健康检查晚于可选代理 relay 启动', () => {
    const sidecarSource = readFileSync(
      new URL('../src/sidecar.ts', import.meta.url),
      'utf8'
    );
    const relayReady = sidecarSource.indexOf(
      'proxyRelay = await startWebviewProxyRelay'
    );
    const healthReady = sidecarSource.indexOf(
      'if (config.apiOnly) installSidecarHealthRoute(apiApp, healthToken);'
    );
    expect(relayReady).toBeGreaterThan(-1);
    expect(healthReady).toBeGreaterThan(relayReady);
  });

  test('父进程管道关闭后只触发一次 Sidecar 自清理', async () => {
    const input = new PassThrough();
    input.write(`${'a'.repeat(64)}\n`);
    expect(await readSidecarHealthToken(input)).toBe('a'.repeat(64));
    let disconnects = 0;
    const stopMonitoring = monitorSidecarParent(input, () => {
      disconnects += 1;
    });

    input.end();
    await new Promise(resolve => setTimeout(resolve, 0));
    stopMonitoring();

    expect(disconnects).toBe(1);
  });

  test('Sidecar 启动后始终监视父进程管道', () => {
    const sidecarSource = readFileSync(
      new URL('../src/sidecar.ts', import.meta.url),
      'utf8'
    );
    expect(sidecarSource).toContain('monitorSidecarParent(input');
  });

  test('管道 EOF 失效时通过父 PID 自清理', async () => {
    let alive = true;
    let disconnects = 0;
    const stopMonitoring = monitorParentProcess(
      123,
      () => {
        disconnects += 1;
      },
      { isAlive: () => alive, intervalMs: 1 }
    );

    await new Promise(resolve => setTimeout(resolve, 2));
    expect(disconnects).toBe(0);
    alive = false;
    await new Promise(resolve => setTimeout(resolve, 5));
    stopMonitoring();

    expect(disconnects).toBe(1);
  });
});
