import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { collectUpdaterArtifacts } from '../scripts/collect-updater-artifacts.mjs';
import { createUpdaterManifest } from '../scripts/generate-updater-manifest.mjs';
import { verifyUpdaterReleaseEnvironment } from '../scripts/verify-updater-release-env.mjs';
import { createUpdaterBuildConfig } from '../scripts/build-tauri-updater.mjs';
import { resolveTauriSmokeExecutable } from '../scripts/smoke-tauri-local.mjs';

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
const rustMain = readFileSync(
  new URL('../src-tauri/src/main.rs', import.meta.url),
  'utf8'
);
const workflow = readFileSync(
  new URL('../.github/workflows/build.yaml', import.meta.url),
  'utf8'
);

test('macOS deployment target matches the proxy API requirement', () => {
  expect(cargo).toContain('"macos-proxy"');
  expect(tauriConfig.bundle.macOS.minimumSystemVersion).toBe('14.0');
});

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
      ['linux-appimage', 'YesPlayMusic_0.7.0_amd64.AppImage'],
      ['linux-deb', 'YesPlayMusic_0.7.0_amd64.deb'],
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
      'linux-x86_64-appimage',
      'linux-x86_64-deb',
      'windows-x86_64',
    ]);
    expect(manifest.platforms['darwin-aarch64']?.url).toEndWith(
      '/v0.7.0/YesPlayMusic.app.tar.gz'
    );
    expect(manifest.platforms['windows-x86_64']?.signature).toBe(
      'YesPlayMusic_0.7.0_x64-setup.exe-signature'
    );
    expect(manifest.platforms['linux-x86_64-appimage']?.url).toEndWith(
      '/v0.7.0/YesPlayMusic_0.7.0_amd64.AppImage'
    );
    expect(manifest.platforms['linux-x86_64-deb']?.url).toEndWith(
      '/v0.7.0/YesPlayMusic_0.7.0_amd64.deb'
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test('Linux updater selects a manifest target from the installed bundle type', () => {
  expect(rustMain).toContain('BundleType::AppImage');
  expect(rustMain).toContain('"linux-x86_64-appimage"');
  expect(rustMain).toContain('BundleType::Deb');
  expect(rustMain).toContain('"linux-x86_64-deb"');
});

test('Linux updater collector preserves both signed package formats', async () => {
  const root = await mkdtemp(
    path.join(tmpdir(), 'yesplaymusic-collector-test-')
  );
  const output = path.join(root, 'output');
  try {
    const bundleRoot = path.join(
      root,
      'src-tauri/target/x86_64-unknown-linux-gnu/release/bundle'
    );
    for (const [directory, name] of [
      ['appimage', 'YesPlayMusic_0.7.0_amd64.AppImage'],
      ['deb', 'YesPlayMusic_0.7.0_amd64.deb'],
    ] as const) {
      const source = path.join(bundleRoot, directory);
      await mkdir(source, { recursive: true });
      await writeFile(path.join(source, name), `${directory}-artifact`);
      await writeFile(path.join(source, `${name}.sig`), `${directory}-sig`);
    }

    const appImage = await collectUpdaterArtifacts(
      'linux-x86_64-appimage',
      output,
      root
    );
    const deb = await collectUpdaterArtifacts('linux-x86_64-deb', output, root);
    expect(appImage.artifactName).toEndWith('.AppImage');
    expect(deb.artifactName).toEndWith('.deb');
    expect(
      await Bun.file(path.join(output, `${appImage.artifactName}.sig`)).text()
    ).toBe('appimage-sig');
    expect(
      await Bun.file(path.join(output, `${deb.artifactName}.sig`)).text()
    ).toBe('deb-sig');
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test('core smoke launches each platform from its packaged runtime layout', async () => {
  const root = await mkdtemp(path.join(tmpdir(), 'yesplaymusic-smoke-test-'));
  try {
    expect(
      resolveTauriSmokeExecutable({ platform: 'darwin', arch: 'arm64', root })
    ).toEndWith(
      'bundle/macos/YesPlayMusic.app/Contents/MacOS/yesplaymusic-tauri'
    );
    expect(
      resolveTauriSmokeExecutable({ platform: 'win32', arch: 'x64', root })
    ).toEndWith('release/yesplaymusic-tauri.exe');

    const appImageDirectory = path.join(
      root,
      'src-tauri/target/x86_64-unknown-linux-gnu/release/bundle/appimage'
    );
    await mkdir(appImageDirectory, { recursive: true });
    await writeFile(
      path.join(appImageDirectory, 'YesPlayMusic_0.7.0_amd64.AppImage'),
      'artifact'
    );
    expect(
      resolveTauriSmokeExecutable({ platform: 'linux', arch: 'x64', root })
    ).toEndWith('bundle/appimage/YesPlayMusic_0.7.0_amd64.AppImage');
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
