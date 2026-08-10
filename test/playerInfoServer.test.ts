import { afterEach, describe, expect, test } from 'bun:test';
import { createServer } from 'node:http';
import type { Server } from 'node:http';
import {
  LEGACY_PLAYER_API_PORT,
  startPlayerInfoServer,
} from '@/services/playerInfoServer';
import { initialPlayerInfo } from '@/services/playerInfo';

const HOST = '127.0.0.1';
const openServers = new Set<Server>();

async function closeServer(server: Server): Promise<void> {
  openServers.delete(server);
  if (!server.listening) return;
  await new Promise<void>((resolve, reject) => {
    server.close(error => (error ? reject(error) : resolve()));
  });
}

function serverPort(server: Server): number {
  const address = server.address();
  if (!address || typeof address === 'string') {
    throw new Error('Expected a TCP listener');
  }
  return address.port;
}

afterEach(async () => {
  await Promise.all([...openServers].map(closeServer));
});

describe('legacy player API listener', () => {
  test('keeps the public compatibility port fixed', () => {
    expect(LEGACY_PLAYER_API_PORT).toBe(27_232);
  });

  test('serves the latest snapshot without caching', async () => {
    let playerInfo = initialPlayerInfo();
    const server = await startPlayerInfoServer({
      readPlayerInfo: () => playerInfo,
      port: 0,
    });
    openServers.add(server);
    const url = `http://${HOST}:${serverPort(server)}/player`;

    const initialResponse = await fetch(url);
    expect(initialResponse.headers.get('cache-control')).toBe('no-store');
    expect(await initialResponse.json()).toEqual({
      currentTrack: null,
      progress: 0,
    });

    playerInfo = { currentTrack: { id: 42, name: 'Track' }, progress: 12.5 };
    expect(await fetch(url).then(response => response.json())).toEqual(
      playerInfo
    );
  });

  test('releases the listener and reports bind failures', async () => {
    const blocker = createServer();
    openServers.add(blocker);
    blocker.listen(0, HOST);
    await new Promise<void>((resolve, reject) => {
      blocker.once('listening', resolve);
      blocker.once('error', reject);
    });
    const port = serverPort(blocker);

    await expect(
      startPlayerInfoServer({ readPlayerInfo: initialPlayerInfo, port })
    ).rejects.toHaveProperty('code', 'EADDRINUSE');

    await closeServer(blocker);
    const replacement = await startPlayerInfoServer({
      readPlayerInfo: initialPlayerInfo,
      port,
    });
    openServers.add(replacement);
    await closeServer(replacement);

    const response = await fetch(`http://${HOST}:${port}/player`).catch(
      () => null
    );
    expect(response).toBeNull();
  });
});
