import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import {
  purgeLegacyDesktopAuthStorage,
  shouldUseLegacyCookieFallback,
} from '../src/utils/authStorage';

const authSource = readFileSync(
  new URL('../src/utils/auth.js', import.meta.url),
  'utf8'
);

function createStorage(entries) {
  const values = new Map(entries);
  return {
    get length() {
      return values.size;
    },
    key(index) {
      return [...values.keys()][index] ?? null;
    },
    removeItem(key) {
      values.delete(key);
    },
    has(key) {
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
});
