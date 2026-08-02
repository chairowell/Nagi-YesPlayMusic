import express from 'express';
import expressProxy from 'express-http-proxy';
import path from 'node:path';
import { randomBytes } from 'node:crypto';
import { startNeteaseMusicApi } from './services/neteaseApi';
import { parseSidecarArgs } from './utils/sidecarConfig';
import {
  addNativeProxyToken,
  hardenAuthCookieHeaders,
  installLocalRequestBoundary,
} from './services/localRequestBoundary';
import { applyRendererSecurityHeaders } from './services/contentSecurityPolicy';

const HOST = '127.0.0.1';

function registerNativeRoutes(apiApp) {
  let unblockMusicService;
  apiApp.post('/native/unblock-music', async (request, response) => {
    const { sourceListString, track, context } = request.body || {};
    if (!track || typeof track !== 'object') {
      response.status(400).send({ message: '缺少歌曲信息' });
      return;
    }

    // 大多数歌曲不会走解锁服务，延迟加载可避免常驻时白白初始化 native addon。
    try {
      if (!unblockMusicService) {
        const { createUnblockMusicService } = await import(
          './services/unblockMusic'
        );
        unblockMusicService = createUnblockMusicService();
      }
      response.send(
        await unblockMusicService(sourceListString, track, context)
      );
    } catch (error) {
      console.error(`[sidecar][UNM] ${error.message}`);
      response.status(500).send(null);
    }
  });
}

async function runUnblockMusicAddonSmokeTest() {
  const { listUnblockMusicSources } = await import('./services/unblockMusic');
  const sources = listUnblockMusicSources();
  if (!sources.length) throw new Error('UNM native addon 没有返回可用音源');
  console.log(`[sidecar][UNM] addon ready: ${sources.join(', ')}`);
}

function startRendererServer(
  { apiPort, webPort, rendererDir },
  { allowedOrigins, nativeToken }
) {
  const app = express();
  installLocalRequestBoundary(app, { allowedOrigins });
  // 正式版页面由 sidecar 的 HTTP origin 提供，必须在真实响应上设置 CSP。
  app.use(applyRendererSecurityHeaders);

  // API 必须和页面同源，否则登录 cookie 会被 WebView 的 SameSite 规则丢弃。
  app.use(
    '/api',
    expressProxy(`http://${HOST}:${apiPort}`, {
      proxyReqOptDecorator(options, request) {
        return addNativeProxyToken(options, request, nativeToken);
      },
      userResHeaderDecorator(headers) {
        return hardenAuthCookieHeaders(headers);
      },
    })
  );
  app.use(express.static(path.resolve(rendererDir)));

  return new Promise((resolve, reject) => {
    const server = app.listen(webPort, HOST, () => resolve(server));
    server.once('error', reject);
  });
}

function closeServer(server) {
  if (!server?.listening) return Promise.resolve();
  return new Promise((resolve, reject) => {
    server.close(error => (error ? reject(error) : resolve()));
  });
}

export async function runSidecar(args = process.argv.slice(2)) {
  const config = parseSidecarArgs(args);
  const allowedOrigins = [
    `http://${HOST}:${config.apiOnly ? 1420 : config.webPort}`,
  ];
  // 上游默认会原样回显任意 Origin；固定该值只是响应层兜底，真正的拒绝发生在边界中间件。
  process.env.CORS_ALLOW_ORIGIN = allowedOrigins[0];
  const nativeToken = config.apiOnly
    ? null
    : randomBytes(32).toString('hex');
  const apiApp = await startNeteaseMusicApi({
    port: config.apiPort,
    host: HOST,
  });
  installLocalRequestBoundary(apiApp, { allowedOrigins, nativeToken });
  registerNativeRoutes(apiApp);
  let rendererServer = null;
  try {
    rendererServer = config.apiOnly
      ? null
      : await startRendererServer(config, { allowedOrigins, nativeToken });
  } catch (error) {
    // UI 端口失败时一并回收 API，避免留下看不见的后台进程。
    await closeServer(apiApp.server);
    throw error;
  }

  console.log(
    `[sidecar] ready: API http://${HOST}:${config.apiPort}` +
      (rendererServer ? `, UI http://${HOST}:${config.webPort}` : '')
  );

  let stopping = false;
  const stop = async signal => {
    if (stopping) return;
    stopping = true;
    console.log(`[sidecar] ${signal} received, shutting down`);
    await Promise.all([
      closeServer(rendererServer),
      closeServer(apiApp.server),
    ]);
    process.exit(0);
  };

  process.once('SIGINT', () => void stop('SIGINT'));
  process.once('SIGTERM', () => void stop('SIGTERM'));

  return { apiServer: apiApp.server, rendererServer };
}

if (import.meta.main) {
  const task = process.argv.includes('--unm-addon-smoke-test')
    ? runUnblockMusicAddonSmokeTest()
    : runSidecar();
  task.catch(error => {
    console.error(`[sidecar] ${error.message}`);
    process.exit(1);
  });
}
