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
const electronIpc = readFileSync(
  new URL('../src/electron/ipcMain.js', import.meta.url),
  'utf8'
);
const electronBackground = readFileSync(
  new URL('../src/background.js', import.meta.url),
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
    expect(normalizeExternalUrl('https://github.com/nagi-studio/YesPlayMusic')).toBe(
      'https://github.com/nagi-studio/YesPlayMusic'
    );
    expect(() => normalizeExternalUrl('javascript:alert(1)')).toThrow(
      '只允许打开 HTTP(S) 外链'
    );
    expect(() => normalizeExternalUrl('file:///etc/passwd')).toThrow(
      '只允许打开 HTTP(S) 外链'
    );
  });

  test('Tauri 使用官方 opener，不再依赖 WebView 的 window.open', async () => {
    const opened = [];
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

  test('Electron 通过主进程调用系统浏览器', async () => {
    const opened = [];
    await openExternalUrl('https://example.com/path', {
      isTauri: false,
      electronOpen: url => opened.push(url),
    });

    expect(opened).toEqual(['https://example.com/path']);
    expect(electronIpc).toContain("['http:', 'https:'].includes(url.protocol)");
    expect(electronIpc).toContain('shell.openExternal(url.href)');
    expect(electronBackground).toContain('setWindowOpenHandler');
    expect(electronBackground).not.toContain("webContents.on('new-window'");
  });

  test('桌面页面里的普通外链点击也统一交给 opener', async () => {
    const opened = [];
    let prevented = false;
    const handler = createExternalLinkClickHandler(url => opened.push(url));
    handler({
      target: {
        closest: () => ({ href: 'https://example.com/docs' }),
      },
      preventDefault: () => {
        prevented = true;
      },
    });
    await Promise.resolve();

    expect(prevented).toBe(true);
    expect(opened).toEqual(['https://example.com/docs']);
  });

  test('不覆盖已被组件处理或非左键的点击', async () => {
    const opened = [];
    const handler = createExternalLinkClickHandler(url => opened.push(url));
    const target = {
      closest: () => ({ href: 'https://example.com/docs' }),
    };

    handler({ defaultPrevented: true, button: 0, target });
    handler({ defaultPrevented: false, button: 1, target });
    await Promise.resolve();

    expect(opened).toEqual([]);
  });
});
