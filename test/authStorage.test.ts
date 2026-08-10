import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import {
  hasAccountSession,
  purgeLegacyDesktopAuthStorage,
  shouldUseLegacyCookieFallback,
} from '../src/utils/authStorage';

const authSource = readFileSync(
  new URL('../src/utils/auth.ts', import.meta.url),
  'utf8'
);

function createStorage(entries: Iterable<readonly [string, string]>) {
  const values = new Map<string, string>(entries);
  return {
    get length() {
      return values.size;
    },
    key(index: number) {
      return [...values.keys()][index] ?? null;
    },
    removeItem(key: string) {
      values.delete(key);
    },
    has(key: string) {
      return values.has(key);
    },
  };
}

describe('桌面登录凭据存储', () => {
  test('桌面端禁止 localStorage 回退，纯 Web 保持兼容', () => {
    expect(shouldUseLegacyCookieFallback(true)).toBe(false);
    expect(shouldUseLegacyCookieFallback(false)).toBe(true);
    expect(authSource).toContain(
      'shouldUseLegacyCookieFallback(isDesktopRuntime)'
    );
  });

  test('启动时只删除历史 cookie 副本，不碰设置和播放器状态', () => {
    const storage = createStorage([
      ['cookie-MUSIC_U', 'secret'],
      ['cookie-__csrf', 'csrf'],
      ['settings', '{}'],
      ['player', '{}'],
    ]);

    expect(purgeLegacyDesktopAuthStorage(storage, true)).toBe(2);
    expect(storage.has('cookie-MUSIC_U')).toBe(false);
    expect(storage.has('cookie-__csrf')).toBe(false);
    expect(storage.has('settings')).toBe(true);
    expect(storage.has('player')).toBe(true);
  });

  test('纯 Web 模式不会删除仍在使用的兼容 Cookie', () => {
    const storage = createStorage([['cookie-MUSIC_U', 'web-secret']]);

    expect(purgeLegacyDesktopAuthStorage(storage, false)).toBe(0);
    expect(storage.has('cookie-MUSIC_U')).toBe(true);
  });

  test('桌面端用已持久化的登录模式判断会话，不需要读取 HttpOnly Cookie', () => {
    expect(
      hasAccountSession({
        isDesktop: true,
        loginMode: 'account',
        readableCookie: undefined,
      })
    ).toBe(true);
    expect(
      hasAccountSession({
        isDesktop: false,
        loginMode: 'account',
        readableCookie: undefined,
      })
    ).toBe(false);
  });
});
