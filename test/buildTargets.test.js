import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

test('macOS 只构建当前支持的 Apple Silicon 架构', () => {
  const config = readFileSync('electron-builder.yml', 'utf8');
  const macBlock = config.slice(config.indexOf('\nmac:'), config.indexOf('\ndmg:'));

  expect(macBlock).toContain('- arm64');
  expect(macBlock).not.toContain('- x64');
});
