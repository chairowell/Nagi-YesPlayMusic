import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { tauriDmgName } from '../scripts/package-tauri-dmg.mjs';
import { verifyAppleReleaseEnvironment } from '../scripts/verify-apple-release-env.mjs';
import {
  validateTauriVersions,
  verifyTauriVersions,
} from '../scripts/verify-tauri-version.mjs';

const workflow = readFileSync(
  new URL('../.github/workflows/build.yaml', import.meta.url),
  'utf8'
);
const readme = readFileSync(
  new URL('../README.md', import.meta.url),
  'utf8'
);
const packageJson = JSON.parse(
  readFileSync(new URL('../package.json', import.meta.url), 'utf8')
);

test('Tauri CI 只构建 Apple Silicon，并保留无签名和签名两条发布路径', () => {
  expect(workflow).toContain('targets: aarch64-apple-darwin');
  expect(workflow).toContain('run: bun run build:tauri');
  expect(workflow).toContain('run: bun run package:tauri:dmg');
  expect(workflow).toContain('run: shasum -a 256 -c *.sha256');
  expect(workflow).toContain('run: bun run verify:tauri:version');
  expect(workflow).toContain('path: dist_tauri/*');
  expect(workflow).toContain('run: bun run build:tauri:release');
  expect(workflow).toContain('run: bun run collect:tauri:release-dmg');
  expect(workflow).not.toContain('build:mac');
  expect(workflow).not.toContain('dist_electron');
});

test('版本 tag 默认走无 Developer ID 签名路径', () => {
  expect(workflow).toContain("vars.APPLE_SIGNING_ENABLED != 'true'");
  expect(workflow).toContain('run: bun run build:tauri');
  expect(workflow).toContain('run: bun run package:tauri:dmg');
});

test('显式开启 Apple 签名后才要求公证和 stapler 验证', () => {
  expect(workflow).toContain("vars.APPLE_SIGNING_ENABLED == 'true'");
  for (const secret of [
    'APPLE_CERTIFICATE',
    'APPLE_CERTIFICATE_PASSWORD',
    'APPLE_SIGNING_IDENTITY',
    'APPLE_ID',
    'APPLE_PASSWORD',
    'APPLE_TEAM_ID',
    'KEYCHAIN_PASSWORD',
  ]) {
    expect(workflow).toContain(`secrets.${secret}`);
  }
  expect(workflow).toContain('xcrun stapler validate');
  expect(workflow).toContain('spctl --assess --type execute');
  expect(packageJson.scripts['build:tauri:release']).toContain('--bundles dmg');
  expect(packageJson.scripts['build:tauri:release']).not.toContain(
    'sign:tauri:local'
  );
});

test('缺少 Apple 发版密钥时在构建前立即失败', () => {
  expect(() => verifyAppleReleaseEnvironment({ APPLE_ID: 'owner@example.com' }))
    .toThrow('APPLE_CERTIFICATE');
  expect(
    verifyAppleReleaseEnvironment({
      APPLE_CERTIFICATE: 'certificate',
      APPLE_CERTIFICATE_PASSWORD: 'certificate-password',
      APPLE_SIGNING_IDENTITY: 'Developer ID Application: Example',
      APPLE_ID: 'owner@example.com',
      APPLE_PASSWORD: 'app-specific-password',
      APPLE_TEAM_ID: 'TEAMID',
      KEYCHAIN_PASSWORD: 'temporary-keychain-password',
    })
  ).toBe(true);
});

test('tag 和三个应用版本字段必须完全一致', () => {
  expect(
    validateTauriVersions({
      packageVersion: '0.6.0',
      tauriVersion: '0.6.0',
      cargoVersion: '0.6.0',
      tag: 'v0.6.0',
    })
  ).toBe('0.6.0');
  expect(() =>
    validateTauriVersions({
      packageVersion: '0.6.0',
      tauriVersion: '0.5.0',
      cargoVersion: '0.6.0',
      tag: 'v0.6.0',
    })
  ).toThrow('版本号不一致');
});

test('本轮 Tauri 重构发布为稳定版', async () => {
  expect(await verifyTauriVersions()).toBe('0.6.0');
});

test('只有版本 tag 获得写权限并创建草稿 release', () => {
  expect(workflow).toContain("if: startsWith(github.ref, 'refs/tags/v')");
  expect(workflow).toContain('contents: write');
  expect(workflow).toContain('draft: true');
});

test('DMG 文件名明确标记版本和 Apple Silicon 架构', () => {
  expect(tauriDmgName('0.6.0')).toBe('YesPlayMusic_0.6.0_aarch64.dmg');
});

test('README 只说明 Apple Silicon 无签名 Tauri 发布方式', () => {
  expect(readme).toContain('bun run build:tauri');
  expect(readme).toContain('bun run package:tauri:dmg');
  expect(readme).not.toContain('Intel 选 `x64`');
  expect(readme).not.toContain('产物在 `dist_electron/`');
});
