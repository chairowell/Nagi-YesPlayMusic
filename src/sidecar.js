import express from 'express';
import expressProxy from 'express-http-proxy';
import path from 'node:path';
import { startNeteaseMusicApi } from './services/neteaseApi';
import { parseSidecarArgs } from './utils/sidecarConfig';

const HOST = '127.0.0.1';

function startRendererServer({ apiPort, webPort, rendererDir }) {
  const app = express();

  // API 必须和页面同源，否则登录 cookie 会被 WebView 的 SameSite 规则丢弃。
  app.use('/api', expressProxy(`http://${HOST}:${apiPort}`));
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
  const apiApp = await startNeteaseMusicApi({
    port: config.apiPort,
    host: HOST,
  });
  let rendererServer = null;
  try {
    rendererServer = config.apiOnly
      ? null
      : await startRendererServer(config);
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
  runSidecar().catch(error => {
    console.error(`[sidecar] ${error.message}`);
    process.exit(1);
  });
}
