import { handleNcmSessionExpiry } from '@/utils/sessionExpiry';

/** Mirrors the sidecar's own 15s resolution deadline. */
const NATIVE_FETCH_TIMEOUT_MS = 15000;

type Fetcher = (
  input: RequestInfo | URL,
  init?: RequestInit
) => Promise<Response>;

export interface NativeFetchOptions {
  fetcher?: Fetcher;
  timeoutMs?: number;
  /** Receives the parsed body of any 401 answer; must validate it itself. */
  onUnauthorized?: (data: unknown) => void;
}

/**
 * Router and auth pull in the whole Vue app, which bun tests must not load
 * just to import a service module — wire the expiry actions lazily.
 */
function defaultOnUnauthorized(data: unknown): void {
  void Promise.all([
    import('@/utils/auth'),
    import('@/router'),
    import('@/utils/runtime'),
  ]).then(([auth, routerModule, runtime]) => {
    handleNcmSessionExpiry(data, {
      loginRoute: runtime.isDesktopRuntime ? 'loginAccount' : 'login',
      logout: auth.doLogout,
      navigate: name => {
        void routerModule.default.push({ name });
      },
    });
  });
}

/**
 * fetch for the typed /native endpoints: a hung network rejects after the
 * timeout instead of leaving the promise pending forever, and a 401 carrying
 * the NCM expiry body ({code:301, msg:"需要登录"}) logs out and returns to the
 * login page. A 401 from the sidecar's own native-token boundary carries a
 * different body and must never clear the session.
 */
export function createNativeFetch(options: NativeFetchOptions = {}): Fetcher {
  const timeoutMs = options.timeoutMs ?? NATIVE_FETCH_TIMEOUT_MS;
  const onUnauthorized = options.onUnauthorized ?? defaultOnUnauthorized;
  return async (input, init) => {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), timeoutMs);
    // Read globalThis.fetch at call time so test stubs keep working.
    const fetcher =
      options.fetcher ?? ((request, extra) => globalThis.fetch(request, extra));
    let response: Response;
    try {
      response = await fetcher(input, { ...init, signal: controller.signal });
    } catch (error) {
      if (controller.signal.aborted) {
        throw new Error(`请求超时（${timeoutMs}ms）：${String(input)}`);
      }
      throw error;
    } finally {
      clearTimeout(timer);
    }
    if (response.status === 401) {
      const data: unknown = await response
        .clone()
        .json()
        .catch(() => null);
      onUnauthorized(data);
    }
    return response;
  };
}

export const nativeFetch = createNativeFetch();
