import { describe, expect, test } from 'bun:test';
import { EventEmitter } from 'node:events';
import { waitForServer } from '../src/utils/serverLifecycle';

describe('后台服务启动握手', () => {
  test('只有实际开始监听后才报告 ready', async () => {
    const server = new EventEmitter();
    let ready = false;
    const waiting = waitForServer(server).then(() => {
      ready = true;
    });

    await Promise.resolve();
    expect(ready).toBe(false);
    server.emit('listening');
    await waiting;
    expect(ready).toBe(true);
  });

  test('监听失败会原样上报并清理事件监听', async () => {
    const server = new EventEmitter();
    const error = new Error('端口不可用');
    const waiting = waitForServer(server);

    server.emit('error', error);
    expect(waiting).rejects.toBe(error);
    await waiting.catch(() => {});
    expect(server.listenerCount('listening')).toBe(0);
    expect(server.listenerCount('error')).toBe(0);
  });
});
