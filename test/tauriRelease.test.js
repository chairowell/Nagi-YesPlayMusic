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
const readme = readFileSync(new URL('../README.md', import.meta.url), 'utf8');
const linuxdeployLdd = readFileSync(
  new URL('../scripts/ci/ldd', import.meta.url),
  'utf8'
);
const packageJson = JSON.parse(
  readFileSync(new URL('../package.json', import.meta.url), 'utf8')
);

test('CI 官方 Actions 使用 Node.js 24 运行时版本', () => {
  for (const action of [
    'actions/checkout@v7',
    'actions/cache@v6',
    'actions/upload-artifact@v7',
    'actions/download-artifact@v8',
  ]) {
    expect(workflow).toContain(action);
  }

  for (const action of [
    'actions/checkout@v4',
    'actions/cache@v4',
    'actions/upload-artifact@v4',
    'actions/download-artifact@v4',
  ]) {
    expect(workflow).not.toContain(action);
  }
});

test('macOS CI 保留无签名和签名两条发布路径', () => {
  const tauriJob = workflow.slice(
    workflow.indexOf('  build-tauri-arm64:'),
    workflow.indexOf('  build-tauri-windows-x64:')
  );
  expect(workflow).toContain('targets: aarch64-apple-darwin');
  expect(workflow).toContain('run: bun run build:tauri');
  expect(workflow).toContain('run: bun run package:tauri:dmg');
  expect(workflow).toContain('run: shasum -a 256 -c ./*.sha256');
  expect(workflow).toContain('run: bun run verify:tauri:version');
  expect(workflow).toContain('path: dist_tauri/*');
  expect(workflow).toContain('run: bun run build:tauri:release');
  expect(workflow).toContain('run: bun run collect:tauri:release-dmg');
  expect(tauriJob).not.toContain('build:mac');
  expect(tauriJob).not.toContain('dist_electron');
});

test('Windows CI 只上传仓库自己 ref 的未签名 x64 测试包', () => {
  const windowsJob = workflow.slice(
    workflow.indexOf('  build-tauri-windows-x64:'),
    workflow.indexOf('  build-tauri-linux-x64:')
  );
  const releaseJob = workflow.slice(workflow.indexOf('  draft-release:'));

  expect(windowsJob).toContain('runs-on: windows-latest');
  expect(windowsJob).toContain('permissions:\n      contents: read');
  expect(windowsJob).toContain(
    'key: bun-target-${{ runner.os }}-1.3.12-windows-x64-baseline'
  );
  expect(windowsJob).toContain(
    'BUN_INSTALL_CACHE_DIR: ${{ runner.temp }}/bun-target-cache'
  );
  expect(windowsJob).toContain(
    'bun install --frozen-lockfile --ignore-scripts'
  );
  expect(windowsJob).toContain('cache-on-failure: true');
  expect(windowsJob).toContain('run: bun run build:tauri:windows');
  expect(windowsJob).toContain(
    'yesplaymusic-sidecar-x86_64-pc-windows-msvc.exe'
  );
  expect(windowsJob).toContain("if: github.event_name != 'pull_request'");
  expect(windowsJob).toContain('Get-FileHash $_.FullName -Algorithm SHA256');
  expect(windowsJob).toContain('dist_tauri_windows/SHA256SUMS.txt');
  expect(windowsJob).toContain("$hashes -join \"`n\"");
  expect(windowsJob).toContain('[System.IO.File]::WriteAllText');
  expect(windowsJob).toContain('dist_tauri_windows/TESTING-NOTICE.txt');
  expect(windowsJob).toContain('Do not disable antivirus');
  expect(windowsJob).toContain('retention-days: 14');
  expect(releaseJob).not.toContain('YesPlayMusic-windows-x64');
});

test('Ubuntu CI 构建 AppImage、deb 并验证目标平台 Sidecar', () => {
  const linuxJob = workflow.slice(
    workflow.indexOf('  build-tauri-linux-x64:'),
    workflow.indexOf('  draft-release:')
  );
  const releaseJob = workflow.slice(workflow.indexOf('  draft-release:'));

  expect(linuxJob).toContain('runs-on: ubuntu-22.04');
  expect(linuxJob).toContain('libwebkit2gtk-4.1-dev');
  expect(linuxJob).toContain('bun run build:tauri:linux');
  expect(linuxJob).toContain('cache-on-failure: true');
  expect(linuxJob).toContain('PATH="$GITHUB_WORKSPACE/scripts/ci:$PATH"');
  expect(linuxdeployLdd).toContain("$(basename \"$target\") == 'yesplaymusic-sidecar'");
  expect(linuxdeployLdd).toContain('/usr/bin/ldd "$@"');
  expect(packageJson.scripts['build:tauri:linux']).toContain('--verbose');
  expect(packageJson.scripts['build:tauri:linux']).toContain(
    '--bundles deb,appimage'
  );
  expect(linuxJob).toContain(
    'yesplaymusic-sidecar-x86_64-unknown-linux-gnu --unm-addon-smoke-test'
  );
  expect(linuxJob).toContain('bundle/appimage/*.AppImage');
  expect(linuxJob).toContain('bundle/deb/*.deb');
  expect(linuxJob).toContain('sha256sum -c SHA256SUMS.txt');
  expect(releaseJob).not.toContain('YesPlayMusic-linux-x64');
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
  expect(() =>
    verifyAppleReleaseEnvironment({ APPLE_ID: 'owner@example.com' })
  ).toThrow('APPLE_CERTIFICATE');
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

test('当前 Tauri 发布保持稳定版', async () => {
  expect(await verifyTauriVersions()).toBe(packageJson.version);
  expect(packageJson.version).not.toContain('-');
});

test('只有版本 tag 获得写权限并创建草稿 release', () => {
  expect(workflow).toContain("if: startsWith(github.ref, 'refs/tags/v')");
  expect(workflow).toContain('contents: write');
  expect(workflow).toContain('draft: true');
});

test('DMG 文件名明确标记版本和 Apple Silicon 架构', () => {
  expect(tauriDmgName('0.6.0')).toBe('YesPlayMusic_0.6.0_aarch64.dmg');
});

test('README 区分 macOS 正式发布与 Windows/Linux 实验构建', () => {
  expect(readme).toContain('macOS Tauri 重构版');
  expect(readme).toContain('381.5 MiB');
  expect(readme).toContain('80.8 MiB');
  expect(readme).toContain('约 79%');
  expect(readme).toContain('docs/performance-baseline.md');
  expect(readme).toContain('bun run build:tauri');
  expect(readme).toContain('bun run package:tauri:dmg');
  expect(readme).toContain('bun run build:tauri:windows');
  expect(readme).toContain('bun run build:tauri:linux');
  expect(readme).toContain('NSIS `.exe`');
  expect(readme).toContain('AppImage');
  expect(readme).not.toContain('Intel 选 `x64`');
  expect(readme).not.toContain('产物在 `dist_electron/`');
});
