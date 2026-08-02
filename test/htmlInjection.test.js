import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import fg from 'fast-glob';
import { stripMarkupToText } from '../src/utils/safeText';

test('所有 Vue 模板禁止把字符串直接交给 v-html', () => {
  const offenders = fg
    .sync('src/**/*.vue')
    .filter(file => readFileSync(file, 'utf8').includes('v-html'));

  expect(offenders).toEqual([]);
});

test('登录提示只保留换行和文字，标签不会进入 DOM', () => {
  expect(
    stripMarkupToText(
      '第一行<br /><img src=x onerror=alert(1)><a href="evil">GitHub</a>'
    )
  ).toBe('第一行\nGitHub');
});
