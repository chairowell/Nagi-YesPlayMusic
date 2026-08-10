import clc from 'cli-color';
// Import for the side effect that creates the anonymous_token file.
import '../utils/checkAuthToken';
import server from '@neteaseapireborn/api/server';
import apiCache from '@neteaseapireborn/api/util/apicache';
// Static imports let Bun collect every route in the standalone sidecar.
import ncmModDef from '../ncmModDef.cjs';
import { configureSafeNeteaseApiCache } from '../utils/audioCacheIntegrity';
import { waitForServer } from '../utils/serverLifecycle';
import type { NcmApiApp } from '@neteaseapireborn/api/server';

configureSafeNeteaseApiCache(apiCache);

export async function startNeteaseMusicApi({
  port = 10754,
  host = '127.0.0.1',
}: { port?: number; host?: string } = {}): Promise<NcmApiApp> {
  console.log(`${clc.redBright('[NetEase API]')} initiating NCM API`);

  const apiApp = await server.serveNcmApi({
    port,
    host,
    moduleDefs: ncmModDef,
  });
  // Wait for listening because upstream resolves before bind failures surface.
  await waitForServer(apiApp.server);
  return apiApp;
}
