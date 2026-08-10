import express from 'express';
import { waitForServer } from '@/utils/serverLifecycle';
import type { Server } from 'node:http';
import type { Application, Request, Response } from 'express';
import type { PlayerInfo } from '@/services/playerInfo';

const LOOPBACK_HOST = '127.0.0.1';

export const LEGACY_PLAYER_API_PORT = 27_232;

export function installPlayerInfoRoute(
  app: Application,
  readPlayerInfo: () => PlayerInfo
): void {
  app.get('/player', (_request: Request, response: Response) => {
    response.setHeader('Cache-Control', 'no-store');
    response.send(readPlayerInfo());
  });
}

export async function startPlayerInfoServer({
  readPlayerInfo,
  port = LEGACY_PLAYER_API_PORT,
}: {
  readPlayerInfo: () => PlayerInfo;
  port?: number;
}): Promise<Server> {
  const app = express();
  app.disable('x-powered-by');
  installPlayerInfoRoute(app, readPlayerInfo);

  const server = app.listen(port, LOOPBACK_HOST);
  try {
    await waitForServer(server);
    return server;
  } catch (error) {
    server.close();
    throw error;
  }
}
