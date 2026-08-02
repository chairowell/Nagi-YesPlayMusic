import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const app = readFileSync(new URL('../src/App.vue', import.meta.url), 'utf8');
const lyrics = readFileSync(
  new URL('../src/views/lyrics.vue', import.meta.url),
  'utf8'
);
const navbar = readFileSync(
  new URL('../src/components/Navbar.vue', import.meta.url),
  'utf8'
);

test('小窗双击进入播放队列，中窗提供明确的返回入口', () => {
  expect(lyrics).toContain('@dblclick="handleMiniDoubleClick"');
  expect(lyrics).toContain("this.$emit('expand-compact-window')");
  expect(app).toContain("this.$router.push({ name: 'next' })");
  expect(navbar).toContain('title="回到迷你播放器"');
  expect(navbar).toContain("$emit('restore-compact-window')");
});
