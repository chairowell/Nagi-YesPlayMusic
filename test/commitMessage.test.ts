import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import {
  COMMIT_TYPES,
  MAX_SUBJECT_LENGTH,
  verifyCommitMessage,
} from '../scripts/verify-commit-message.mjs';

const hook = readFileSync(
  new URL('../.githooks/commit-msg', import.meta.url),
  'utf8'
);

test('每个类型都配一个 emoji，且 emoji 不重复', () => {
  const emojis = Object.values(COMMIT_TYPES);
  expect(emojis.length).toBe(new Set(emojis).size);
  expect(Object.keys(COMMIT_TYPES)).toContain('release');
});

test('合规标题按类型、scope 和破坏性标记解析', () => {
  expect(verifyCommitMessage('🐛 fix: 迷你播放条双击不再最大化')).toMatchObject({
    skipped: false,
    type: 'fix',
    scope: null,
    breaking: false,
    description: '迷你播放条双击不再最大化',
  });
  expect(
    verifyCommitMessage('♻️ refactor(sidecar): 拆分路由注册')
  ).toMatchObject({ type: 'refactor', scope: 'sidecar' });
  expect(verifyCommitMessage('✨ feat!: 换掉设置文件格式')).toMatchObject({
    type: 'feat',
    breaking: true,
  });
  expect(verifyCommitMessage('🔖 release: 0.8.0-canary.1')).toMatchObject({
    type: 'release',
  });
});

test('缺前缀、类型不存在或 emoji 配错都拦下来', () => {
  expect(() => verifyCommitMessage('修复了一个问题')).toThrow('标题格式');
  expect(() => verifyCommitMessage('fix: 少了 emoji')).toThrow('标题格式');
  expect(() => verifyCommitMessage('🐛 bugfix: 类型不在白名单')).toThrow(
    '未知类型'
  );
  expect(() => verifyCommitMessage('🐛 feat: emoji 和类型对不上')).toThrow(
    '应该配'
  );
});

test('emoji 带不带变体选择符都接受', () => {
  expect(verifyCommitMessage('♻️ refactor: 带变体选择符')).toMatchObject({
    type: 'refactor',
  });
  expect(verifyCommitMessage('♻ refactor: 不带变体选择符')).toMatchObject({
    type: 'refactor',
  });
});

test('标题过长、结尾句号、正文没空行都拦下来', () => {
  const long = `🐛 fix: ${'长'.repeat(MAX_SUBJECT_LENGTH)}`;
  expect(() => verifyCommitMessage(long)).toThrow('超过');
  expect(() => verifyCommitMessage('🐛 fix: 结尾有句号。')).toThrow('句号');
  expect(() => verifyCommitMessage('🐛 fix: 标题\n正文贴着标题')).toThrow(
    '空一行'
  );
  expect(
    verifyCommitMessage('🐛 fix: 标题\n\n正文和标题隔了一行')
  ).toMatchObject({ type: 'fix' });
});

test('git 自己生成的标题和注释行不参与校验', () => {
  for (const generated of [
    "Merge branch 'master' into feature",
    'Revert "🐛 fix: 某个改动"',
    'fixup! 🐛 fix: 某个改动',
  ]) {
    expect(verifyCommitMessage(generated)).toEqual({ skipped: true });
  }
  expect(
    verifyCommitMessage('# 注释在前\n🐛 fix: 注释不影响解析')
  ).toMatchObject({ type: 'fix' });
});

test('rebase 或 merge 进行中时钩子直接放行', () => {
  expect(hook).toContain('rebase-merge');
  expect(hook).toContain('rebase-apply');
  expect(hook).toContain('bun scripts/verify-commit-message.mjs "$1"');
});
