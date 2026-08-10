import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

test('Tauri 实际处理迷你模式的 macOS 红绿灯显隐事件', () => {
  const mainSource = readFileSync(
    fileURLToPath(new URL('../src-tauri/src/main.rs', import.meta.url)),
    'utf8'
  );

  expect(mainSource).toContain('fn set_window_button_visibility');
  expect(mainSource).toMatch(
    /"setWindowButtonVisibility"\s*=>[\s\S]*set_window_button_visibility/
  );
  expect(mainSource).not.toMatch(
    /"setProxy"[\s\S]*\|\s*"setWindowButtonVisibility"/
  );
});
