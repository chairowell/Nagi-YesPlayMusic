import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { tauriDmgName } from '../scripts/package-tauri-dmg.mjs';
import {
  validateTauriVersions,
  verifyTauriVersions,
} from '../scripts/verify-tauri-version.mjs';

const workflow = readFileSync(
  new URL('../.github/workflows/build.yaml', import.meta.url),
  'utf8'
);

test('Tauri CI 只构建 Apple Silicon 并上传签名后的 DMG', () => {
  expect(workflow).toContain('targets: aarch64-apple-darwin');
  expect(workflow).toContain('run: bun run build:tauri');
  expect(workflow).toContain('run: bun run package:tauri:dmg');
  expect(workflow).toContain('run: shasum -a 256 -c *.sha256');
  expect(workflow).toContain('run: bun run verify:tauri:version');
  expect(workflow).toContain('path: dist_tauri/*');
  expect(workflow).not.toContain('build:mac');
  expect(workflow).not.toContain('dist_electron');
});

test('tag 和三个应用版本字段必须完全一致', () => {
  expect(
    validateTauriVersions({
      packageVersion: '0.6.0-beta.1',
      tauriVersion: '0.6.0-beta.1',
      cargoVersion: '0.6.0-beta.1',
      tag: 'v0.6.0-beta.1',
    })
  ).toBe('0.6.0-beta.1');
  expect(() =>
    validateTauriVersions({
      packageVersion: '0.6.0',
      tauriVersion: '0.5.0',
      cargoVersion: '0.6.0',
      tag: 'v0.6.0',
    })
  ).toThrow('版本号不一致');
});

test('本轮 Tauri 重构使用独立 beta 版本线', async () => {
  expect(await verifyTauriVersions()).toBe('0.6.0-beta.1');
});

test('只有版本 tag 获得写权限并创建草稿 release', () => {
  expect(workflow).toContain("if: startsWith(github.ref, 'refs/tags/v')");
  expect(workflow).toContain('contents: write');
  expect(workflow).toContain('draft: true');
});

test('DMG 文件名明确标记版本和 Apple Silicon 架构', () => {
  expect(tauriDmgName('0.6.0-beta.1')).toBe(
    'YesPlayMusic_0.6.0-beta.1_aarch64.dmg'
  );
});
