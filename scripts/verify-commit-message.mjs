#!/usr/bin/env bun
// Commit subjects are "<emoji> <type>: <subject>" — gitmoji for scanning,
// the type prefix so `git log --oneline | grep '^.. fix:'` still works.
import { readFileSync } from 'node:fs';

// Each type owns exactly one emoji; a mismatched pair carries no information.
export const COMMIT_TYPES = {
  feat: '✨',
  fix: '🐛',
  docs: '📝',
  test: '✅',
  ci: '👷',
  refactor: '♻️',
  perf: '⚡️',
  chore: '🔧',
  revert: '⏪️',
  release: '🔖',
};

export const MAX_SUBJECT_LENGTH = 72;

// Git writes these itself; rejecting them only blocks merges and rebases.
const GENERATED_SUBJECT = /^(Merge |Revert |fixup!|squash!|amend!)/;

const VARIATION_SELECTOR = /️/g;

function normalizeEmoji(value) {
  return value.replace(VARIATION_SELECTOR, '');
}

function stripComments(text) {
  return text
    .split('\n')
    .filter(line => !line.startsWith('#'))
    .join('\n');
}

export function verifyCommitMessage(rawText) {
  const text = stripComments(rawText).replace(/^\s*\n+/, '');
  const lines = text.split('\n');
  const subject = (lines[0] ?? '').trimEnd();

  if (!subject) throw new Error('提交信息不能为空');
  if (GENERATED_SUBJECT.test(subject)) return { skipped: true };

  const match = subject.match(
    /^(\S+) ([a-z]+)(?:\(([a-z0-9-]+)\))?(!)?: (.+)$/u
  );
  if (!match) {
    throw new Error(
      `标题格式应为「<emoji> <类型>: <描述>」，例如「${COMMIT_TYPES.fix} fix: 修好了 XXX」\n收到：${subject}`
    );
  }

  const [, emoji, type, scope, breaking, description] = match;
  const expected = COMMIT_TYPES[type];
  if (!expected) {
    throw new Error(
      `未知类型「${type}」。可用类型：${Object.keys(COMMIT_TYPES).join('、')}`
    );
  }
  if (normalizeEmoji(emoji) !== normalizeEmoji(expected)) {
    throw new Error(`「${type}」应该配 ${expected}，收到 ${emoji}`);
  }
  if ([...subject].length > MAX_SUBJECT_LENGTH) {
    throw new Error(
      `标题 ${[...subject].length} 个字符，超过 ${MAX_SUBJECT_LENGTH}，请精简或挪进正文`
    );
  }
  if (description.endsWith('。') || description.endsWith('.')) {
    throw new Error('标题结尾不要句号');
  }
  if (lines.length > 1 && lines[1]?.trim() !== '') {
    throw new Error('标题和正文之间要空一行');
  }

  return {
    skipped: false,
    emoji,
    type,
    scope: scope ?? null,
    breaking: Boolean(breaking),
    description,
  };
}

if (import.meta.main) {
  const messagePath = process.argv[2];
  if (!messagePath) {
    console.error('[commit-msg] 缺少提交信息文件路径');
    process.exit(1);
  }
  try {
    verifyCommitMessage(readFileSync(messagePath, 'utf8'));
  } catch (error) {
    console.error(`[commit-msg] ${error.message}`);
    console.error('');
    console.error('可用类型与对应 emoji：');
    for (const [type, emoji] of Object.entries(COMMIT_TYPES)) {
      console.error(`  ${emoji} ${type}`);
    }
    process.exit(1);
  }
}
