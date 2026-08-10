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
import { installPreciseWavRoutes } from './services/preciseWav';
import {
  installSidecarHealthRoute,
  readSidecarHealthToken,
} from './services/sidecarIdentity';
import { installDesktopLogoutRoute } from './services/sidecarSession';
import { startWebviewProxyRelay } from './services/webviewProxyRelay';
import { decodePlayerInfo, initialPlayerInfo } from './services/playerInfo';
import {
  installPlayerInfoRoute,
  LEGACY_PLAYER_API_PORT,
  startPlayerInfoServer,
} from './services/playerInfoServer';
import type { Application, Request, Response } from 'express';
import type { Server } from 'node:http';
import type { Readable } from 'node:stream';
import type { NcmApiApp } from '@neteaseapireborn/api/server';
import type { SidecarConfig } from './utils/sidecarConfig';
import type { Track } from './types/domain';
import type { ProxyRelay } from './services/webviewProxyRelay';
import type { PlayerInfo } from './services/playerInfo';
import type { createUnblockMusicService as CreateUnblockMusicService } from './services/unblockMusic';

const HOST = '127.0.0.1';

function isUnknownRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function hasOptionalString(
  record: Record<string, unknown>,
  key: string
): boolean {
  return record[key] === undefined || typeof record[key] === 'string';
}

function hasOptionalFiniteNumber(
  record: Record<string, unknown>,
  key: string
): boolean {
  const value = record[key];
  return (
    value === undefined || (typeof value === 'number' && Number.isFinite(value))
  );
}

function isTrack(value: unknown): value is Track {
  if (!isUnknownRecord(value)) return false;
  if (typeof value['id'] !== 'number' || !Number.isFinite(value['id'])) {
    return false;
  }
  if (
    !hasOptionalString(value, 'name') ||
    !hasOptionalFiniteNumber(value, 'dt')
  ) {
    return false;
  }

  const album = value['al'];
  if (
    album !== undefined &&
    (!isUnknownRecord(album) ||
      typeof album['id'] !== 'number' ||
      !Number.isFinite(album['id']) ||
      !hasOptionalString(album, 'name'))
  ) {
    return false;
  }

  const artists = value['ar'];
  return (
    artists === undefined ||
    (Array.isArray(artists) &&
      artists.every(
        artist =>
          isUnknownRecord(artist) &&
          typeof artist['id'] === 'number' &&
          Number.isFinite(artist['id']) &&
          hasOptionalString(artist, 'name')
      ))
  );
}

function registerNativeRoutes(
  apiApp: Application,
  apiPort: number,
  updatePlayerInfo: (value: unknown) => boolean
): void {
  let unblockMusicService: ReturnType<typeof CreateUnblockMusicService> | null =
    null;
  installDesktopLogoutRoute(apiApp, apiPort);

  apiApp.post('/native/player-info', (request: Request, response: Response) => {
    if (!updatePlayerInfo(request.body)) {
      response.status(400).send({ message: '播放器状态无效' });
      return;
    }
    response.status(204).end();
  });

  apiApp.post(
    '/native/unblock-music',
    async (request: Request, response: Response) => {
      const body: unknown = request.body;
      const payload = isUnknownRecord(body) ? body : {};
      const sourceListString = payload['sourceListString'];
      const track = payload['track'];
      const context = payload['context'];
      if (!isTrack(track)) {
        response.status(400).send({ message: '缺少歌曲信息' });
        return;
      }

      // Most tracks skip UNM, so load the native addon only when needed.
      try {
        if (!unblockMusicService) {
          const { createUnblockMusicService } = await import(
            './services/unblockMusic'
          );
          unblockMusicService = createUnblockMusicService();
        }
        response.send(
          await unblockMusicService(
            sourceListString,
            track,
            isUnknownRecord(context) ? context : {}
          )
        );
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        console.error(`[sidecar][UNM] ${message}`);
        response.status(500).send(null);
      }
    }
  );
}

async function runUnblockMusicAddonSmokeTest(): Promise<void> {
  const { listUnblockMusicSources } = await import('./services/unblockMusic');
  const sources = listUnblockMusicSources();
  if (!sources.length) throw new Error('UNM native addon 没有返回可用音源');
  console.log(`[sidecar][UNM] addon ready: ${sources.join(', ')}`);
}

function startRendererServer(
  {
    apiPort,
    webPort,
    rendererDir,
  }: Pick<SidecarConfig, 'apiPort' | 'webPort'> & { rendererDir: string },
  {
    allowedOrigins,
    nativeToken,
    healthToken,
    readPlayerInfo,
  }: {
    allowedOrigins: string[];
    nativeToken: string;
    healthToken: string;
    readPlayerInfo: () => PlayerInfo;
  }
): Promise<Server> {
  const app = express();
  installSidecarHealthRoute(app, healthToken);
  installLocalRequestBoundary(app, { allowedOrigins });
  // Production pages need CSP on the sidecar's actual HTTP response.
  app.use(applyRendererSecurityHeaders);
  installPlayerInfoRoute(app, readPlayerInfo);

  // Keep the API same-origin so WebView SameSite rules preserve login cookies.
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
  // Stream FLAC through afconvert and serve WAV ranges for precise, low-memory seeks.
  installPreciseWavRoutes(app);
  app.use(express.static(path.resolve(rendererDir)));

  return new Promise<Server>((resolve, reject) => {
    const server = app.listen(webPort, HOST, () => resolve(server));
    server.once('error', reject);
  });
}

function closeServer(server: Server | null | undefined): Promise<void> {
  if (!server?.listening) return Promise.resolve();
  return new Promise<void>((resolve, reject) => {
    server.close(error => (error ? reject(error) : resolve()));
  });
}

export async function runSidecar(
  args: string[] = process.argv.slice(2),
  input: Readable = process.stdin
): Promise<{
  apiServer: Server;
  rendererServer: Server | null;
  playerServer: Server;
  proxyServer: Server | null;
}> {
  const config = parseSidecarArgs(args);
  // Pass the token through an anonymous stdin pipe to keep it out of process metadata.
  const healthToken = await readSidecarHealthToken(input);
  const allowedOrigins = [
    `http://${HOST}:${config.apiOnly ? 1420 : config.webPort}`,
  ];
  // Pin the reflected Origin as defense in depth; the boundary middleware rejects it.
  process.env['CORS_ALLOW_ORIGIN'] = allowedOrigins[0];
  const nativeToken = config.apiOnly ? null : randomBytes(32).toString('hex');
  let playerInfo = initialPlayerInfo();
  const updatePlayerInfo = (value: unknown): boolean => {
    const decoded = decodePlayerInfo(value);
    if (!decoded) return false;
    playerInfo = decoded;
    return true;
  };
  const apiApp: NcmApiApp = await startNeteaseMusicApi({
    port: config.apiPort,
    host: HOST,
  });
  installLocalRequestBoundary(apiApp, { allowedOrigins, nativeToken });
  registerNativeRoutes(apiApp, config.apiPort, updatePlayerInfo);
  let rendererServer: Server | null = null;
  let playerServer: Server | null = null;
  let proxyRelay: ProxyRelay | null = null;
  try {
    if (config.upstreamProxy) {
      proxyRelay = await startWebviewProxyRelay({
        port: config.proxyRelayPort,
        upstreamProxy: config.upstreamProxy,
      });
    }
    playerServer = await startPlayerInfoServer({
      readPlayerInfo: () => playerInfo,
    });
    if (!config.apiOnly) {
      if (!config.rendererDir || !nativeToken) {
        throw new Error('renderer server 配置不完整');
      }
      rendererServer = await startRendererServer(
        { ...config, rendererDir: config.rendererDir },
        {
          allowedOrigins,
          nativeToken,
          healthToken,
          readPlayerInfo: () => playerInfo,
        }
      );
    }
  } catch (error) {
    // Stop every live listener when startup fails.
    await Promise.all([
      closeServer(rendererServer),
      closeServer(playerServer),
      proxyRelay?.close(),
      closeServer(apiApp.server),
    ]);
    throw error;
  }

  if (!playerServer) {
    await Promise.all([proxyRelay?.close(), closeServer(apiApp.server)]);
    throw new Error('播放器兼容 API 未启动');
  }

  // Report ready only after every required local listener is live.
  if (config.apiOnly) installSidecarHealthRoute(apiApp, healthToken);

  console.log(
    `[sidecar] ready: API http://${HOST}:${config.apiPort}` +
      (rendererServer ? `, UI http://${HOST}:${config.webPort}` : '') +
      `, player http://${HOST}:${LEGACY_PLAYER_API_PORT}/player` +
      (proxyRelay ? `, proxy http://${HOST}:${config.proxyRelayPort}` : '')
  );

  let stopping = false;
  const stop = async (signal: NodeJS.Signals): Promise<void> => {
    if (stopping) return;
    stopping = true;
    console.log(`[sidecar] ${signal} received, shutting down`);
    await Promise.all([
      closeServer(rendererServer),
      closeServer(playerServer),
      closeServer(apiApp.server),
      proxyRelay?.close(),
    ]);
    process.exit(0);
  };

  process.once('SIGINT', () => void stop('SIGINT'));
  process.once('SIGTERM', () => void stop('SIGTERM'));

  return {
    apiServer: apiApp.server,
    rendererServer,
    playerServer,
    proxyServer: proxyRelay?.server ?? null,
  };
}

if (import.meta.main) {
  const task = process.argv.includes('--unm-addon-smoke-test')
    ? runUnblockMusicAddonSmokeTest()
    : runSidecar();
  task.catch((error: unknown) => {
    const message = error instanceof Error ? error.message : String(error);
    console.error(`[sidecar] ${message}`);
    process.exit(1);
  });
}
