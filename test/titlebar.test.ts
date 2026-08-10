import { expect, test } from 'bun:test';
import { resolveCustomTitlebar } from '../src/services/titlebar';

test('Tauri restores platform titlebars without affecting the web app', () => {
  expect(resolveCustomTitlebar('win32', true, false)).toBe('windows');
  expect(resolveCustomTitlebar('linux', true, true)).toBe('linux');
  expect(resolveCustomTitlebar('linux', true, false)).toBeNull();
  expect(resolveCustomTitlebar('linux', false, true)).toBeNull();
  expect(resolveCustomTitlebar('darwin', true, true)).toBeNull();
});
