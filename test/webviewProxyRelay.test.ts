import { afterEach, describe, expect, test } from 'bun:test';
import http from 'node:http';
import net from 'node:net';
import {
  isLoopbackProxyHost,
  parseUpstreamProxy,
  startWebviewProxyRelay,
} from '../src/services/webviewProxyRelay';
import type { AddressInfo } from 'node:net';
import type { ProxyRelay } from '../src/services/webviewProxyRelay';

const HOST = '127.0.0.1';
const servers: Array<http.Server | net.Server> = [];
const relays: ProxyRelay[] = [];

function appendBuffer(current: Buffer, chunk: Buffer): Buffer {
  const combined = Buffer.allocUnsafe(current.length + chunk.length);
  for (let index = 0; index < current.length; index += 1) {
    combined[index] = current[index] ?? 0;
  }
  for (let index = 0; index < chunk.length; index += 1) {
    combined[current.length + index] = chunk[index] ?? 0;
  }
  return combined;
}

function listen(server: http.Server | net.Server): Promise<number> {
  servers.push(server);
  return new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(0, HOST, () => {
      server.off('error', reject);
      resolve((server.address() as AddressInfo).port);
    });
  });
}

async function reservePort(): Promise<number> {
  const server = net.createServer();
  const port = await listen(server);
  await closeServer(server);
  servers.splice(servers.indexOf(server), 1);
  return port;
}

function closeServer(server: http.Server | net.Server): Promise<void> {
  if (!server.listening) return Promise.resolve();
  return new Promise((resolve, reject) => {
    server.close(error => (error ? reject(error) : resolve()));
  });
}

function proxyGet(port: number, target: string): Promise<string> {
  return new Promise((resolve, reject) => {
    const request = http.get(
      {
        host: HOST,
        port,
        path: target,
        headers: { host: new URL(target).host },
      },
      response => {
        response.setEncoding('utf8');
        let body = '';
        response.on('data', chunk => {
          body += chunk;
        });
        response.on('end', () => resolve(body));
      }
    );
    request.once('error', reject);
  });
}

function rawProxyGet(port: number, target: string): Promise<string> {
  return new Promise((resolve, reject) => {
    const socket = net.connect(port, HOST);
    let received = '';
    socket.once('connect', () => {
      socket.write(
        `GET ${target} HTTP/1.1\r\nHost: ${
          new URL(target).host
        }\r\nConnection: close\r\n\r\n`
      );
    });
    socket.setEncoding('utf8');
    socket.on('data', chunk => {
      received += chunk;
    });
    socket.once('end', () => resolve(received));
    socket.once('error', reject);
  });
}

function connectTunnel(
  relayPort: number,
  authority: string
): Promise<{ socket: net.Socket; remainder: Buffer }> {
  return new Promise((resolve, reject) => {
    const socket = net.connect(relayPort, HOST);
    let received = Buffer.alloc(0);
    socket.once('connect', () => {
      socket.write(
        `CONNECT ${authority} HTTP/1.1\r\nHost: ${authority}\r\n\r\n`
      );
    });
    const onData = (chunk: Buffer): void => {
      received = appendBuffer(received, chunk);
      const boundary = received.indexOf('\r\n\r\n');
      if (boundary < 0) return;
      socket.off('data', onData);
      const header = received.subarray(0, boundary).toString('utf8');
      if (!header.includes(' 200 ')) {
        reject(new Error(`CONNECT failed: ${header}`));
        socket.destroy();
        return;
      }
      resolve({ socket, remainder: received.subarray(boundary + 4) });
    };
    socket.on('data', onData);
    socket.once('error', reject);
  });
}

function exchange(
  socket: net.Socket,
  initial: Buffer,
  payload: string
): Promise<string> {
  return new Promise((resolve, reject) => {
    let received = initial;
    const finish = (): void => {
      if (received.length < Buffer.byteLength(payload)) return;
      cleanup();
      resolve(received.subarray(0, Buffer.byteLength(payload)).toString());
    };
    const onData = (chunk: Buffer): void => {
      received = appendBuffer(received, chunk);
      finish();
    };
    const onError = (error: Error): void => {
      cleanup();
      reject(error);
    };
    const cleanup = (): void => {
      socket.off('data', onData);
      socket.off('error', onError);
    };
    socket.on('data', onData);
    socket.on('error', onError);
    socket.write(payload);
    finish();
  });
}

async function startRelay(upstreamPort: number): Promise<ProxyRelay> {
  const port = await reservePort();
  const relay = await startWebviewProxyRelay({
    port,
    upstreamProxy: `http://${HOST}:${upstreamPort}`,
  });
  relays.push(relay);
  return relay;
}

afterEach(async () => {
  await Promise.all(relays.splice(0).map(relay => relay.close()));
  await Promise.all(servers.splice(0).map(server => closeServer(server)));
});

describe('WebView 本地代理 relay', () => {
  test('严格拒绝认证、路径、查询和非 HTTP upstream', () => {
    expect(parseUpstreamProxy('http://proxy.example:8080').href).toBe(
      'http://proxy.example:8080/'
    );
    for (const value of [
      'https://proxy.example:8080',
      'http://user@proxy.example:8080',
      'http://proxy.example:8080/path',
      'http://proxy.example:8080?mode=1',
      'http://proxy.example:8080?',
      'http://proxy.example:8080#',
      'http://proxy.example:0',
      ' http://proxy.example:8080',
    ]) {
      expect(() => parseUpstreamProxy(value)).toThrow();
    }
  });

  test('只把明确的 loopback 主机判为直连', () => {
    expect(isLoopbackProxyHost('127.0.0.1')).toBe(true);
    expect(isLoopbackProxyHost('localhost')).toBe(true);
    expect(isLoopbackProxyHost('[::1]')).toBe(true);
    expect(isLoopbackProxyHost('127.0.0.2')).toBe(false);
    expect(isLoopbackProxyHost('localhost.example')).toBe(false);
  });

  test('外部 HTTP absolute-form 原样交给 upstream proxy', async () => {
    let requestTarget = '';
    const upstreamPort = await listen(
      http.createServer((request, response) => {
        requestTarget = request.url ?? '';
        response.end('via-upstream');
      })
    );
    const relay = await startRelay(upstreamPort);
    const relayPort = (relay.server.address() as AddressInfo).port;
    const target = 'http://outside.invalid:8123/music?id=7%2F8';

    expect(await rawProxyGet(relayPort, target)).toContain('via-upstream');
    expect(requestTarget).toBe(target);
  });

  test('127.0.0.1 和 localhost HTTP 目标直连', async () => {
    let upstreamRequests = 0;
    const upstreamPort = await listen(
      http.createServer((_request, response) => {
        upstreamRequests += 1;
        response.statusCode = 502;
        response.end();
      })
    );
    const targetPort = await listen(
      http.createServer((request, response) => {
        response.end(request.url);
      })
    );
    const relay = await startRelay(upstreamPort);
    const relayPort = (relay.server.address() as AddressInfo).port;

    expect(
      await proxyGet(relayPort, `http://127.0.0.1:${targetPort}/one?q=1`)
    ).toBe('/one?q=1');
    expect(
      await proxyGet(relayPort, `http://localhost:${targetPort}/two?q=2`)
    ).toBe('/two?q=2');
    expect(upstreamRequests).toBe(0);
  });

  test('外部 CONNECT 由 upstream 建隧道', async () => {
    let authority = '';
    const upstream = http.createServer();
    upstream.on('connect', (request, client, head) => {
      authority = request.url ?? '';
      client.write('HTTP/1.1 200 Connection Established\r\n\r\n');
      if (head.length) client.write(head);
      client.pipe(client);
    });
    const upstreamPort = await listen(upstream);
    const relay = await startRelay(upstreamPort);
    const relayPort = (relay.server.address() as AddressInfo).port;

    const tunnel = await connectTunnel(relayPort, 'outside.invalid:443');
    expect(await exchange(tunnel.socket, tunnel.remainder, 'hello')).toBe(
      'hello'
    );
    expect(authority).toBe('outside.invalid:443');
    tunnel.socket.destroy();
  });

  test('loopback CONNECT 直连，relay shutdown 同时关闭隧道', async () => {
    let upstreamConnects = 0;
    const upstream = http.createServer();
    upstream.on('connect', (_request, client) => {
      upstreamConnects += 1;
      client.destroy();
    });
    const upstreamPort = await listen(upstream);
    const targetPort = await listen(
      net.createServer(socket => {
        socket.pipe(socket);
      })
    );
    const relay = await startRelay(upstreamPort);
    const relayPort = (relay.server.address() as AddressInfo).port;
    const tunnel = await connectTunnel(relayPort, `127.0.0.1:${targetPort}`);

    expect(await exchange(tunnel.socket, tunnel.remainder, 'direct')).toBe(
      'direct'
    );
    expect(upstreamConnects).toBe(0);
    const closed = new Promise<void>(resolve => {
      tunnel.socket.once('close', () => resolve());
    });
    await relay.close();
    await closed;
    expect(relay.server.listening).toBe(false);
  });
});
