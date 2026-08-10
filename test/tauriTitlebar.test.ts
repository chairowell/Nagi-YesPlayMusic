import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { parse } from '@vue/compiler-sfc';
import { NodeTypes } from '@vue/compiler-core';
import type { ElementNode, TemplateChildNode } from '@vue/compiler-core';

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
  expect(main).toContain('#[cfg(target_os = "macos")]');
  expect(main).toContain('.title_bar_style(tauri::TitleBarStyle::Overlay)');
  expect(main).toContain('.hidden_title(true)');
  expect(main).toContain('#[cfg(target_os = "windows")]');
  expect(main).toContain('builder.decorations(false)');
  expect(navbar).toContain('data-tauri-drag-region');
  expect(lyrics).toContain('data-tauri-drag-region');
});

test('Windows 自定义标题栏通过统一桌面桥控制 Tauri 窗口', () => {
  const win32Titlebar = readFileSync(
    new URL('../src/components/Win32Titlebar.vue', import.meta.url),
    'utf8'
  );
  expect(win32Titlebar).toContain('data-tauri-drag-region');
  expect(win32Titlebar).toContain("sendDesktop('minimize')");
  expect(win32Titlebar).toContain("sendDesktop('maximizeOrUnmaximize')");
  expect(win32Titlebar).toContain("sendDesktop('close')");
  expect(main).toContain('"maximizeOrUnmaximize" =>');
  expect(main).toContain('desktop://isMaximized');
  expect(main).toMatch(
    /WindowEvent::Resized\(_\)[\s\S]*emit_maximized_state\(app, &window_for_events\)/
  );
});

test('repeat 和 shuffle 菜单保留旧版原生快捷键', () => {
  expect(main).toMatch(/"app\.repeat",\s*"Repeat",\s*true,\s*Some\("Alt\+R"\)/);
  expect(main).toMatch(
    /"app\.shuffle",\s*"Shuffle",\s*true,\s*Some\("Alt\+S"\)/
  );
});

test('顶栏里铺满整行的容器都要自己带拖拽标记', () => {
  // Tauri checks the mousedown target itself. Each flex container must carry
  // the drag marker so empty navbar space remains draggable.
  const { descriptor } = parse(navbar, { filename: 'Navbar.vue' });
  const template = descriptor.template;
  if (!template?.ast) throw new Error('Navbar.vue 缺少 template AST');
  const isElement = (node: TemplateChildNode): node is ElementNode =>
    node.type === NodeTypes.ELEMENT;
  const rootElement = template.ast.children.find(isElement);
  const navElement = rootElement?.children.find(
    (node): node is ElementNode => isElement(node) && node.tag === 'nav'
  );
  if (!navElement) throw new Error('Navbar.vue 缺少 nav 根容器');

  const elementChildren = navElement.children.filter(isElement);
  expect(elementChildren.length).toBeGreaterThan(0);
  for (const child of elementChildren) {
    // Platform titlebars own their buttons and are not drag regions.
    if (child.tag.endsWith('Titlebar')) continue;
    const names = child.props.map(prop => prop.name);
    expect({
      tag: child.tag,
      hasDragRegion: names.includes('data-tauri-drag-region'),
    }).toEqual({ tag: child.tag, hasDragRegion: true });
  }
});
