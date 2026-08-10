import { desktopSessionExpiryCookies } from '@/services/sidecarIdentity';
import type { Application, Request, Response } from 'express';

const HOST = '127.0.0.1';

export function installDesktopLogoutRoute(
  apiApp: Pick<Application, 'post'>,
  apiPort: number,
  requestLogout: typeof fetch = fetch
): void {
  apiApp.post(
    '/native/logout-session',
    (request: Request, response: Response) => {
      // Expire the local HttpOnly session even when remote logout fails.
      const cookie = request.headers.cookie;
      void requestLogout(`http://${HOST}:${apiPort}/logout`, {
        method: 'POST',
        headers: cookie ? { Cookie: cookie } : {},
      }).catch((error: unknown) => {
        const message = error instanceof Error ? error.message : String(error);
        console.warn(`[sidecar][logout] 远端注销失败：${message}`);
      });
      // Use 204 so upstream never caches the logout response or Set-Cookie.
      response.statusCode = 204;
      response.setHeader('Set-Cookie', desktopSessionExpiryCookies());
      response.setHeader('Cache-Control', 'no-store');
      response.end();
    }
  );
}
