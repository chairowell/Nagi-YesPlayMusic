import { buildLastfmAuthorizationUrl } from '@/api/lastfm';
import { isTauriRuntime } from '@/utils/runtime';
import type { LastfmState } from '@/types/domain';
import type { Event as TauriEvent } from '@tauri-apps/api/event';
import type { WebviewWindow as TauriWebviewWindow } from '@tauri-apps/api/webviewWindow';

export const LASTFM_AUTH_WINDOW_LABEL = 'lastfm-auth';
export const LASTFM_AUTH_EVENT = 'lastfm://authorized';

const MAIN_WINDOW_LABEL = 'main';
const WINDOW_DESTROYED_EVENT = 'tauri://destroyed';
const WINDOW_ERROR_EVENT = 'tauri://error';

type Unlisten = () => void;
type WindowEventHandler = (payload: unknown) => void;

export interface AuthorizedLastfmSession extends LastfmState {
  key: string;
}

export interface LastfmAuthWindowHandle {
  once(event: string, handler: WindowEventHandler): Promise<Unlisten>;
  close(): Promise<void>;
}

interface LastfmAuthWindowOptions {
  url: string;
  title: string;
  width: number;
  height: number;
  center: boolean;
  resizable: boolean;
}

export interface LastfmAuthRuntime {
  listenForAuthorization(handler: WindowEventHandler): Promise<Unlisten>;
  getWindowByLabel(label: string): Promise<LastfmAuthWindowHandle | null>;
  createWindow(
    label: string,
    options: LastfmAuthWindowOptions
  ): LastfmAuthWindowHandle;
  currentWindowLabel(): string;
  emitToMain(payload: AuthorizedLastfmSession): Promise<void>;
  closeCurrentWindow(): Promise<void>;
}

interface LastfmAuthorizationCallbacks {
  onAuthorized(session: AuthorizedLastfmSession): void;
  onError(error: Error): void;
}

interface LastfmAuthorizationOptions {
  origin?: string;
  runtime?: LastfmAuthRuntime;
}

function errorFromPayload(payload: unknown): Error {
  return new Error(
    typeof payload === 'string'
      ? payload
      : 'Unable to create the Last.fm authorization window'
  );
}

export function decodeAuthorizedLastfmSession(
  value: unknown
): AuthorizedLastfmSession {
  if (
    typeof value !== 'object' ||
    value === null ||
    Array.isArray(value) ||
    !('key' in value) ||
    typeof value.key !== 'string' ||
    value.key.length === 0
  ) {
    throw new Error('Last.fm returned an invalid session');
  }
  return { ...value, key: value.key };
}

export function persistAuthorizedLastfmSession(
  value: unknown,
  storage: Pick<Storage, 'setItem'>
): AuthorizedLastfmSession {
  const session = decodeAuthorizedLastfmSession(value);
  storage.setItem('lastfm', JSON.stringify(session));
  return session;
}

export function isLastfmCallbackLocation(
  location: Pick<Location, 'pathname' | 'hash'>
): boolean {
  return (
    location.pathname === '/lastfm/callback' ||
    /^#\/lastfm\/callback(?:\?|$)/.test(location.hash)
  );
}

async function loadLastfmAuthRuntime(): Promise<LastfmAuthRuntime> {
  const [{ emitTo, listen }, { getCurrentWebviewWindow, WebviewWindow }] =
    await Promise.all([
      import('@tauri-apps/api/event'),
      import('@tauri-apps/api/webviewWindow'),
    ]);

  const wrapWindow = (window: TauriWebviewWindow): LastfmAuthWindowHandle => ({
    once: (event, handler) =>
      window.once<unknown>(event, (received: TauriEvent<unknown>) =>
        handler(received.payload)
      ),
    close: () => window.close(),
  });

  return {
    listenForAuthorization: handler =>
      listen<unknown>(LASTFM_AUTH_EVENT, event => handler(event.payload), {
        target: { kind: 'WebviewWindow', label: MAIN_WINDOW_LABEL },
      }),
    async getWindowByLabel(label) {
      const existing = await WebviewWindow.getByLabel(label);
      return existing === null ? null : wrapWindow(existing);
    },
    createWindow(label, options) {
      return wrapWindow(new WebviewWindow(label, options));
    },
    currentWindowLabel: () => getCurrentWebviewWindow().label,
    emitToMain: payload =>
      emitTo(
        { kind: 'WebviewWindow', label: MAIN_WINDOW_LABEL },
        LASTFM_AUTH_EVENT,
        payload
      ),
    closeCurrentWindow: () => getCurrentWebviewWindow().close(),
  };
}

export async function startDesktopLastfmAuthorization(
  callbacks: LastfmAuthorizationCallbacks,
  {
    origin = window.location.origin,
    runtime: providedRuntime,
  }: LastfmAuthorizationOptions = {}
): Promise<Unlisten> {
  const runtime = providedRuntime ?? (await loadLastfmAuthRuntime());
  const disposers: Unlisten[] = [];
  let active = true;
  let authWindow: LastfmAuthWindowHandle | null = null;

  const finish = (closeWindow: boolean) => {
    if (!active) return;
    active = false;
    disposers.splice(0).forEach(dispose => dispose());
    if (closeWindow && authWindow !== null) {
      void authWindow.close().catch(error => {
        console.warn(
          '[lastfm] failed to close the authorization window',
          error
        );
      });
    }
  };

  try {
    disposers.push(
      await runtime.listenForAuthorization(payload => {
        if (!active) return;
        let session: AuthorizedLastfmSession;
        try {
          session = decodeAuthorizedLastfmSession(payload);
        } catch (error) {
          callbacks.onError(
            error instanceof Error ? error : new Error(String(error))
          );
          return;
        }
        finish(true);
        callbacks.onAuthorized(session);
      })
    );

    const existing = await runtime.getWindowByLabel(LASTFM_AUTH_WINDOW_LABEL);
    if (existing !== null) await existing.close();

    authWindow = runtime.createWindow(LASTFM_AUTH_WINDOW_LABEL, {
      url: buildLastfmAuthorizationUrl({ desktop: true, origin }),
      title: 'Connect Last.fm',
      width: 520,
      height: 720,
      center: true,
      resizable: true,
    });
    disposers.push(
      await authWindow.once(WINDOW_ERROR_EVENT, payload => {
        finish(false);
        callbacks.onError(errorFromPayload(payload));
      })
    );
    disposers.push(
      await authWindow.once(WINDOW_DESTROYED_EVENT, () => finish(false))
    );
  } catch (error) {
    finish(false);
    throw error;
  }

  return () => finish(true);
}

export async function publishDesktopLastfmAuthorization(
  value: unknown,
  providedRuntime?: LastfmAuthRuntime
): Promise<boolean> {
  if (!providedRuntime && !isTauriRuntime) return false;
  const runtime = providedRuntime ?? (await loadLastfmAuthRuntime());
  if (runtime.currentWindowLabel() !== LASTFM_AUTH_WINDOW_LABEL) return false;
  await runtime.emitToMain(decodeAuthorizedLastfmSession(value));
  return true;
}

export async function closeLastfmAuthorizationWindow(
  providedRuntime?: LastfmAuthRuntime
): Promise<boolean> {
  if (!providedRuntime && !isTauriRuntime) return false;
  const runtime = providedRuntime ?? (await loadLastfmAuthRuntime());
  if (runtime.currentWindowLabel() !== LASTFM_AUTH_WINDOW_LABEL) return false;
  await runtime.closeCurrentWindow();
  return true;
}
