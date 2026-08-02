import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

test('窄窗口解除旧版 768px 页面宽度，并禁止 WebKit 页面级横向滚动条', () => {
  const globalStyles = readFileSync(
    fileURLToPath(new URL('../src/assets/css/global.scss', import.meta.url)),
    'utf8'
  );

  expect(globalStyles).toMatch(/html\s*\{[^}]*overflow-x:\s*hidden/s);
  expect(globalStyles).toMatch(
    /@media\s*\(max-width:\s*767px\)[\s\S]*?html\s*\{[^}]*min-width:\s*0/s
  );
});
