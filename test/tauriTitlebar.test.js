import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { parse } from '@vue/compiler-sfc';

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

test('顶栏里铺满整行的容器都要自己带拖拽标记', () => {
  // Tauri 只认 mousedown 目标自身的属性，不像 Electron 的 app-region 按几何
  // 区域算。三个 flex:1 的容器把 nav 挤成上下两条细边，漏标一个，那一列的
  // 空白就只会选中文字而不是拖窗口。
  const { descriptor } = parse(navbar, { filename: 'Navbar.vue' });
  const found = [];
  const walk = node => {
    if (node.tag === 'nav') found.push(node);
    (node.children || []).forEach(walk);
  };
  walk(descriptor.template.ast);
  expect(found).toHaveLength(1);

  const elementChildren = found[0].children.filter(node => node.type === 1);
  expect(elementChildren.length).toBeGreaterThan(0);
  for (const child of elementChildren) {
    // 平台专属标题栏是组件，自带按钮，不参与拖拽
    if (child.tag.endsWith('Titlebar')) continue;
    const names = child.props.map(prop => prop.name);
    expect({
      tag: child.tag,
      hasDragRegion: names.includes('data-tauri-drag-region'),
    }).toEqual({ tag: child.tag, hasDragRegion: true });
  }
});
