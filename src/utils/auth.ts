import Cookies from 'js-cookie';
import { clearDesktopSession, logout } from '@/api/auth';
import { getAppStore } from '@/stores/accessor';
import { isDesktopRuntime, isTauriRuntime } from '@/utils/runtime';
import {
  hasAccountSession,
  shouldUseLegacyCookieFallback,
} from '@/utils/authStorage';
import { performLogout } from '@/utils/logout';

export function setCookies(value: string): void {
  if (!shouldUseLegacyCookieFallback(isDesktopRuntime)) return;
  const cookies = value.split(';;');
  cookies.forEach(cookie => {
    document.cookie = cookie;
    const cookieKeyValue = (cookie.split(';')[0] ?? '').split('=');
    const key = cookieKeyValue[0];
    const cookieValue = cookieKeyValue[1];
    if (key && cookieValue !== undefined) {
      localStorage.setItem(`cookie-${key}`, cookieValue);
    }
  });
}

export function getCookie(key: string): string | undefined {
  const cookie = Cookies.get(key);
  if (cookie !== undefined) return cookie;
  return shouldUseLegacyCookieFallback(isDesktopRuntime)
    ? localStorage.getItem(`cookie-${key}`) ?? undefined
    : undefined;
}

export function removeCookie(key: string): void {
  Cookies.remove(key);
  if (shouldUseLegacyCookieFallback(isDesktopRuntime)) {
    localStorage.removeItem(`cookie-${key}`);
  }
}

// MUSIC_U exists only for account sessions.
export function isLoggedIn(): boolean {
  const store = getAppStore();
  return hasAccountSession({
    isDesktop: isDesktopRuntime,
    loginMode: store.data.loginMode,
    readableCookie: getCookie('MUSIC_U'),
  });
}

// Account session.
export function isAccountLoggedIn(): boolean {
  return isLoggedIn();
}

// Read-only username lookup.
export function isUsernameLoggedIn(): boolean {
  return getAppStore().data.loginMode === 'username';
}

// Accept account sessions and read-only username lookups.
export function isLooseLoggedIn(): boolean {
  return isAccountLoggedIn() || isUsernameLoggedIn();
}

let logoutTask: Promise<boolean> | null = null;

export function doLogout(): Promise<boolean> {
  if (logoutTask) return logoutTask;
  logoutTask = performLogout(getAppStore(), {
    isTauri: isTauriRuntime,
    clearDesktopSession,
    requestWebLogout: () => void logout(),
    removeWebCookie: removeCookie,
    reportError: error => console.error('[auth] 注销失败', error),
  }).finally(() => {
    logoutTask = null;
  });
  return logoutTask;
}
