import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import {
  buildLastfmAuthorizationUrl,
  readLastfmCallbackToken,
} from '../src/api/lastfm';
import {
  LASTFM_AUTH_EVENT,
  LASTFM_AUTH_WINDOW_LABEL,
  persistAuthorizedLastfmSession,
  publishDesktopLastfmAuthorization,
  startDesktopLastfmAuthorization,
} from '../src/services/lastfmAuth';
import type {
  LastfmAuthRuntime,
  LastfmAuthWindowHandle,
} from '../src/services/lastfmAuth';

function createWindowHandle() {
  const listeners = new Map<string, (payload: unknown) => void>();
  let closeCount = 0;
  const handle: LastfmAuthWindowHandle = {
    async once(event, handler) {
      listeners.set(event, handler);
      return () => listeners.delete(event);
    },
    async close() {
      closeCount += 1;
    },
  };
  return {
    handle,
    listeners,
    get closeCount() {
      return closeCount;
    },
  };
}

describe('Last.fm OAuth', () => {
  test('authorization URL keeps a fixed Last.fm origin and app callback', () => {
    const url = new URL(
      buildLastfmAuthorizationUrl({
        apiKey: 'test-key',
        desktop: true,
        origin: 'http://127.0.0.1:28232',
      })
    );

    expect(url.origin).toBe('https://www.last.fm');
    expect(url.pathname).toBe('/api/auth/');
    expect(url.searchParams.get('api_key')).toBe('test-key');
    expect(url.searchParams.get('cb')).toBe(
      'http://127.0.0.1:28232/#/lastfm/callback'
    );
    expect(() =>
      buildLastfmAuthorizationUrl({
        apiKey: 'test-key',
        desktop: true,
        origin: 'https://example.com',
      })
    ).toThrow('Last.fm desktop callback origin is not allowed');
  });

  test('reads Last.fm tokens from either side of a hash route', () => {
    expect(
      readLastfmCallbackToken({
        search: '?token=query-token',
        hash: '#/lastfm/callback',
      })
    ).toBe('query-token');
    expect(
      readLastfmCallbackToken({
        search: '',
        hash: '#/lastfm/callback?token=hash-token',
      })
    ).toBe('hash-token');
  });

  test('main WebView receives and validates the child-window session', async () => {
    const authWindow = createWindowHandle();
    const authorizationListener: {
      current: ((payload: unknown) => void) | null;
    } = { current: null };
    let createdLabel = '';
    let createdUrl = '';
    let receivedKey = '';
    const stored = new Map<string, string>();
    let unlistenCount = 0;
    const runtime: LastfmAuthRuntime = {
      async listenForAuthorization(handler) {
        authorizationListener.current = handler;
        return () => {
          unlistenCount += 1;
        };
      },
      async getWindowByLabel() {
        return null;
      },
      createWindow(label, options) {
        createdLabel = label;
        createdUrl = options.url;
        return authWindow.handle;
      },
      currentWindowLabel() {
        return 'main';
      },
      async emitToMain() {},
      async closeCurrentWindow() {},
    };

    await startDesktopLastfmAuthorization(
      {
        onAuthorized: session => {
          const persisted = persistAuthorizedLastfmSession(session, {
            setItem: (key, value) => stored.set(key, value),
          });
          receivedKey = persisted.key;
        },
        onError: error => {
          throw error;
        },
      },
      { runtime, origin: 'http://127.0.0.1:28232' }
    );

    expect(createdLabel).toBe(LASTFM_AUTH_WINDOW_LABEL);
    expect(new URL(createdUrl).origin).toBe('https://www.last.fm');
    expect(authorizationListener.current).not.toBeNull();
    const notify = authorizationListener.current;
    if (notify === null) {
      throw new Error('authorization listener was not registered');
    }
    notify({ key: 'session-key', name: 'listener' });
    await Promise.resolve();

    expect(receivedKey).toBe('session-key');
    expect(JSON.parse(stored.get('lastfm') ?? '')).toEqual({
      key: 'session-key',
      name: 'listener',
    });
    expect(authWindow.closeCount).toBe(1);
    expect(unlistenCount).toBe(1);
  });

  test('callback emits only from the fixed authorization window', async () => {
    const emitted: unknown[] = [];
    const runtime: LastfmAuthRuntime = {
      async listenForAuthorization() {
        return () => {};
      },
      async getWindowByLabel() {
        return null;
      },
      createWindow() {
        return createWindowHandle().handle;
      },
      currentWindowLabel() {
        return LASTFM_AUTH_WINDOW_LABEL;
      },
      async emitToMain(payload) {
        emitted.push(payload);
      },
      async closeCurrentWindow() {},
    };

    expect(
      await publishDesktopLastfmAuthorization(
        { key: 'session-key', name: 'listener' },
        runtime
      )
    ).toBe(true);
    expect(emitted).toEqual([{ key: 'session-key', name: 'listener' }]);
    expect(LASTFM_AUTH_EVENT).toBe('lastfm://authorized');

    runtime.currentWindowLabel = () => 'main';
    expect(
      await publishDesktopLastfmAuthorization({ key: 'another-key' }, runtime)
    ).toBe(false);
    expect(emitted).toHaveLength(1);
  });

  test('capabilities isolate the remote authorization page from IPC', () => {
    const mainCapability = JSON.parse(
      readFileSync(
        new URL('../src-tauri/capabilities/default.json', import.meta.url),
        'utf8'
      )
    ) as { permissions: unknown[] };
    const authCapability = JSON.parse(
      readFileSync(
        new URL('../src-tauri/capabilities/lastfm-auth.json', import.meta.url),
        'utf8'
      )
    ) as {
      windows: string[];
      remote: { urls: string[] };
      permissions: string[];
    };

    expect(mainCapability.permissions).toContain(
      'core:webview:allow-create-webview-window'
    );
    expect(authCapability.windows).toEqual([LASTFM_AUTH_WINDOW_LABEL]);
    expect(authCapability.remote.urls).toEqual([
      'http://127.0.0.1:1420/*',
      'http://127.0.0.1:28232/*',
    ]);
    expect(authCapability.permissions).toEqual([
      'core:event:allow-emit-to',
      'core:window:allow-close',
    ]);
  });
});
