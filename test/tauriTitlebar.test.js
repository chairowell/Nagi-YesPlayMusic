import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const main = readFileSync(
  new URL('../src-tauri/src/main.rs', import.meta.url),
  'utf8'
);
const navbar = readFileSync(
  new URL('../src/components/Navbar.vue', import.meta.url),
  'utf8'
);
const lyrics = readFileSync(
  new URL('../src/views/lyrics.vue', import.meta.url),
  'utf8'
);

test('Tauri 使用隐藏标题的覆盖式标题栏，让歌词背景延伸到窗口顶边', () => {
  expect(main).toContain('.title_bar_style(tauri::TitleBarStyle::Overlay)');
  expect(main).toContain('.hidden_title(true)');
  expect(navbar).toContain('data-tauri-drag-region');
  expect(lyrics).toContain('data-tauri-drag-region');
});
