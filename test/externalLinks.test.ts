import { describe, expect, test } from 'bun:test';
import {
  createExternalLinkClickHandler,
  normalizeExternalUrl,
  openExternalUrl,
} from '../src/services/externalLinks';
import { readFileSync } from 'node:fs';

const tauriMain = readFileSync(
  new URL('../src-tauri/src/main.rs', import.meta.url),
  'utf8'
);
const desktopBridge = readFileSync(
  new URL('../src/services/desktopBridge.ts', import.meta.url),
  'utf8'
);
const capabilities = JSON.parse(
  readFileSync(
    new URL('../src-tauri/capabilities/default.json', import.meta.url),
    'utf8'
  )
);

describe('桌面外链', () => {
  test('只允许交给系统浏览器打开 HTTP(S) 地址', () => {
    expect(
      normalizeExternalUrl('https://github.com/nagi-studio/YesPlayMusic')
    ).toBe('https://github.com/nagi-studio/YesPlayMusic');
    expect(() => normalizeExternalUrl('javascript:alert(1)')).toThrow(
      '只允许打开 HTTP(S) 外链'
    );
    expect(() => normalizeExternalUrl('file:///etc/passwd')).toThrow(
      '只允许打开 HTTP(S) 外链'
    );
  });

  test('Tauri 使用官方 opener，不再依赖 WebView 的 window.open', async () => {
    const opened: string[] = [];
    await openExternalUrl('https://example.com/path', {
      isTauri: true,
      tauriOpen: url => opened.push(url),
    });
    expect(opened).toEqual(['https://example.com/path']);
    expect(tauriMain).toContain('.plugin(tauri_plugin_opener::init())');
    expect(capabilities.permissions).toContainEqual({
      identifier: 'opener:allow-open-url',
      allow: [{ url: 'https://*' }, { url: 'http://*' }],
    });
  });

  test('桌面页面里的普通外链点击也统一交给 opener', async () => {
    const opened: string[] = [];
    let prevented = false;
    const handler = createExternalLinkClickHandler(url => opened.push(url));
    const event = {
      defaultPrevented: false,
      button: 0,
      type: 'click',
      target: {
        closest: () => ({ href: 'https://example.com/docs' }),
      },
      preventDefault: () => {
        prevented = true;
      },
    };
    handler(event);
    await Promise.resolve();

    expect(prevented).toBe(true);
    expect(opened).toEqual(['https://example.com/docs']);
  });

  test('中键和带 modifier 的新窗口链接只交给 opener 一次', async () => {
    const opened: string[] = [];
    const handler = createExternalLinkClickHandler(url => opened.push(url));
    let prevented = 0;
    const target = {
      closest: () => ({
        href: 'https://example.com/docs',
        target: '_blank',
      }),
    };

    handler({
      defaultPrevented: false,
      button: 0,
      metaKey: true,
      type: 'click',
      target,
      preventDefault: () => {
        prevented += 1;
      },
    });
    handler({
      defaultPrevented: false,
      button: 1,
      type: 'auxclick',
      target,
      preventDefault: () => {
        prevented += 1;
      },
    });
    await Promise.resolve();

    expect(prevented).toBe(2);
    expect(opened).toEqual([
      'https://example.com/docs',
      'https://example.com/docs',
    ]);
    expect(desktopBridge).toContain(
      "document.addEventListener('auxclick', externalLinkClick)"
    );
    expect(desktopBridge).toContain(
      "document.removeEventListener('auxclick', externalLinkClick)"
    );
  });

  test('不覆盖已处理、右键或非 HTTP(S) 点击', async () => {
    const opened: string[] = [];
    const handler = createExternalLinkClickHandler(url => opened.push(url));
    let prevented = false;
    const target = {
      closest: () => ({ href: 'https://example.com/docs' }),
    };

    handler({
      defaultPrevented: true,
      button: 0,
      type: 'click',
      target,
      preventDefault: () => {
        prevented = true;
      },
    });
    handler({
      defaultPrevented: false,
      button: 2,
      type: 'auxclick',
      target,
      preventDefault: () => {
        prevented = true;
      },
    });
    handler({
      defaultPrevented: false,
      button: 0,
      type: 'click',
      target: {
        closest: () => ({ href: 'file:///tmp/example' }),
      },
      preventDefault: () => {
        prevented = true;
      },
    });
    await Promise.resolve();

    expect(prevented).toBe(false);
    expect(opened).toEqual([]);
  });
});
