import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { createUpdaterManifest } from '../scripts/generate-updater-manifest.mjs';
import { verifyUpdaterReleaseEnvironment } from '../scripts/verify-updater-release-env.mjs';
import { createUpdaterBuildConfig } from '../scripts/build-tauri-updater.mjs';

const packageJson = JSON.parse(
  readFileSync(new URL('../package.json', import.meta.url), 'utf8')
);
const tauriConfig = JSON.parse(
  readFileSync(new URL('../src-tauri/tauri.conf.json', import.meta.url), 'utf8')
);
const updaterConfig = JSON.parse(
  readFileSync(
    new URL('../src-tauri/tauri.updater.conf.json', import.meta.url),
    'utf8'
  )
);
const capabilities = JSON.parse(
  readFileSync(
    new URL('../src-tauri/capabilities/default.json', import.meta.url),
    'utf8'
  )
);
const cargo = readFileSync(
  new URL('../src-tauri/Cargo.toml', import.meta.url),
  'utf8'
);
const workflow = readFileSync(
  new URL('../.github/workflows/build.yaml', import.meta.url),
  'utf8'
);

test('desktop builds use the official Tauri updater and process plugins', () => {
  expect(packageJson.dependencies['@tauri-apps/plugin-updater']).toBeTruthy();
  expect(packageJson.dependencies['@tauri-apps/plugin-process']).toBeTruthy();
  expect(cargo).toContain('tauri-plugin-updater = "2"');
  expect(cargo).toContain('tauri-plugin-process = "2"');
  expect(capabilities.permissions).toContain('updater:default');
  expect(capabilities.permissions).toContain('process:allow-restart');
  expect(tauriConfig.plugins.updater.endpoints).toEqual([
    'https://github.com/nagi-studio/YesPlayMusic/releases/latest/download/latest.json',
  ]);
  expect(tauriConfig.bundle.createUpdaterArtifacts).toBe(false);
  expect(updaterConfig.bundle.createUpdaterArtifacts).toBe(true);
});

test('tag CI signs all updater targets and publishes latest.json', () => {
  expect(workflow).toContain('secrets.TAURI_SIGNING_PRIVATE_KEY');
  expect(workflow).toContain('vars.TAURI_UPDATER_PUBKEY');
  expect(workflow).toContain('build:tauri:macos:updater');
  expect(workflow).toContain('build:tauri:windows:updater');
  expect(workflow).toContain('build:tauri:linux:updater');
  expect(workflow).toContain('verify-macos-updater.mjs');
  expect(workflow).toContain('generate-updater-manifest.mjs');
  expect(workflow).toContain('release/latest.json');
});

test('release builds inject the public key into createUpdaterArtifacts config', async () => {
  const config = await createUpdaterBuildConfig('public-key');
  expect(config).toMatchObject({
    bundle: { createUpdaterArtifacts: true },
    plugins: { updater: { pubkey: 'public-key' } },
  });
});

test('updater release configuration requires a private and public key', () => {
  expect(() => verifyUpdaterReleaseEnvironment({})).toThrow(
    'TAURI_SIGNING_PRIVATE_KEY, TAURI_SIGNING_PRIVATE_KEY_PASSWORD, TAURI_UPDATER_PUBKEY'
  );
  expect(
    verifyUpdaterReleaseEnvironment({
      TAURI_SIGNING_PRIVATE_KEY: 'private-key',
      TAURI_SIGNING_PRIVATE_KEY_PASSWORD: 'private-key-password',
      TAURI_UPDATER_PUBKEY: 'public-key',
    })
  ).toBe(true);
});

test('latest.json maps every supported target to its signed release asset', async () => {
  const root = await mkdtemp(path.join(tmpdir(), 'yesplaymusic-updater-test-'));
  try {
    const artifacts: Array<[string, string]> = [
      ['macos', 'YesPlayMusic.app.tar.gz'],
      ['windows', 'YesPlayMusic_0.7.0_x64-setup.exe'],
      ['linux', 'YesPlayMusic_0.7.0_amd64.AppImage'],
    ];
    for (const [directory, name] of artifacts) {
      const target = path.join(root, directory);
      await mkdir(target, { recursive: true });
      await writeFile(path.join(target, name), 'artifact');
      await writeFile(path.join(target, `${name}.sig`), `${name}-signature`);
    }

    const manifest = await createUpdaterManifest({
      artifactsDir: root,
      version: '0.7.0',
      publishedAt: '2026-08-10T00:00:00Z',
    });
    expect(Object.keys(manifest.platforms).sort()).toEqual([
      'darwin-aarch64',
      'linux-x86_64',
      'windows-x86_64',
    ]);
    expect(manifest.platforms['darwin-aarch64']?.url).toEndWith(
      '/v0.7.0/YesPlayMusic.app.tar.gz'
    );
    expect(manifest.platforms['windows-x86_64']?.signature).toBe(
      'YesPlayMusic_0.7.0_x64-setup.exe-signature'
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
