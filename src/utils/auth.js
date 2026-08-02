import Cookies from 'js-cookie';
import { logout } from '@/api/auth';
import store from '@/store';
import { isDesktopRuntime } from '@/utils/runtime';
import {
  hasAccountSession,
  shouldUseLegacyCookieFallback,
} from '@/utils/authStorage';

export function setCookies(string) {
  if (!shouldUseLegacyCookieFallback(isDesktopRuntime)) return;
  const cookies = string.split(';;');
  cookies.map(cookie => {
    document.cookie = cookie;
    const cookieKeyValue = cookie.split(';')[0].split('=');
    localStorage.setItem(`cookie-${cookieKeyValue[0]}`, cookieKeyValue[1]);
  });
}

export function getCookie(key) {
  const cookie = Cookies.get(key);
  if (cookie !== undefined) return cookie;
  return shouldUseLegacyCookieFallback(isDesktopRuntime)
    ? localStorage.getItem(`cookie-${key}`)
    : undefined;
}

export function removeCookie(key) {
  Cookies.remove(key);
  if (shouldUseLegacyCookieFallback(isDesktopRuntime)) {
    localStorage.removeItem(`cookie-${key}`);
  }
}

// MUSIC_U 只有在账户登录的情况下才有
export function isLoggedIn() {
  return hasAccountSession({
    isDesktop: isDesktopRuntime,
    loginMode: store.state.data.loginMode,
    readableCookie: getCookie('MUSIC_U'),
  });
}

// 账号登录
export function isAccountLoggedIn() {
  return isLoggedIn();
}

// 用户名搜索（用户数据为只读）
export function isUsernameLoggedIn() {
  return store.state.data.loginMode === 'username';
}

// 账户登录或者用户名搜索都判断为登录，宽松检查
export function isLooseLoggedIn() {
  return isAccountLoggedIn() || isUsernameLoggedIn();
}

export function doLogout() {
  logout();
  removeCookie('MUSIC_U');
  removeCookie('__csrf');
  // 更新状态仓库中的用户信息
  store.commit('updateData', { key: 'user', value: {} });
  // 更新状态仓库中的登录状态
  store.commit('updateData', { key: 'loginMode', value: null });
  // 更新状态仓库中的喜欢列表
  store.commit('updateData', { key: 'likedSongPlaylistID', value: undefined });
}
