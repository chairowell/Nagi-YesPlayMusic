import { createHash } from 'node:crypto';
import { createReadStream } from 'node:fs';
import {
  access,
  copyFile,
  mkdir,
  mkdtemp,
  rm,
  symlink,
  writeFile,
} from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const projectRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '..'
);

export const defaultTauriAppPath = path.join(
  projectRoot,
  'src-tauri/target/aarch64-apple-darwin/release/bundle/macos/YesPlayMusic.app'
);

export function tauriDmgName(version) {
  return `YesPlayMusic_${version}_aarch64.dmg`;
}

export function tauriBundledDmgPath(version) {
  return path.join(
    projectRoot,
    'src-tauri/target/aarch64-apple-darwin/release/bundle/dmg',
    tauriDmgName(version)
  );
}

function run(command, args) {
  const result = Bun.spawnSync([command, ...args], {
    stdout: 'inherit',
    stderr: 'inherit',
  });
  if (result.exitCode !== 0) {
    throw new Error(`${command} 执行失败（退出码 ${result.exitCode}）`);
  }
}

async function sha256(file) {
  const hash = createHash('sha256');
  for await (const chunk of createReadStream(file)) hash.update(chunk);
  return hash.digest('hex');
}

async function writeChecksum(file) {
  const checksumPath = `${file}.sha256`;
  const checksum = await sha256(file);
  await writeFile(
    checksumPath,
    `${checksum}  ${path.basename(file)}\n`,
    'utf8'
  );
  return checksumPath;
}

export async function packageTauriDmg({
  appPath = defaultTauriAppPath,
  outputDir = path.join(projectRoot, 'dist_tauri'),
} = {}) {
  await access(appPath);
  const pkg = await Bun.file(path.join(projectRoot, 'package.json')).json();
  const dmgPath = path.join(outputDir, tauriDmgName(pkg.version));
  const checksumPath = `${dmgPath}.sha256`;
  const stagingDir = await mkdtemp(path.join(tmpdir(), 'yesplaymusic-dmg-'));

  try {
    await mkdir(outputDir, { recursive: true });
    await rm(dmgPath, { force: true });
    await rm(checksumPath, { force: true });
    run('codesign', ['--verify', '--deep', '--strict', '--verbose=2', appPath]);
    run('ditto', [appPath, path.join(stagingDir, 'YesPlayMusic.app')]);
    await symlink('/Applications', path.join(stagingDir, 'Applications'));
    run('hdiutil', [
      'create',
      '-volname',
      'YesPlayMusic',
      '-srcfolder',
      stagingDir,
      '-ov',
      '-format',
      'UDZO',
      dmgPath,
    ]);
    run('hdiutil', ['verify', dmgPath]);

    await writeChecksum(dmgPath);
    return { dmgPath, checksumPath };
  } finally {
    await rm(stagingDir, { recursive: true, force: true });
  }
}

export async function collectTauriReleaseDmg({
  sourcePath,
  appPath = defaultTauriAppPath,
  outputDir = path.join(projectRoot, 'dist_tauri'),
} = {}) {
  const pkg = await Bun.file(path.join(projectRoot, 'package.json')).json();
  const resolvedSource = sourcePath || tauriBundledDmgPath(pkg.version);
  const dmgPath = path.join(outputDir, tauriDmgName(pkg.version));

  await access(resolvedSource);
  await access(appPath);
  await mkdir(outputDir, { recursive: true });
  await rm(dmgPath, { force: true });
  await rm(`${dmgPath}.sha256`, { force: true });
  run('codesign', ['--verify', '--deep', '--strict', '--verbose=2', appPath]);
  run('hdiutil', ['verify', resolvedSource]);
  await copyFile(resolvedSource, dmgPath);
  const checksumPath = await writeChecksum(dmgPath);
  return { dmgPath, checksumPath };
}

if (import.meta.main) {
  const result = process.argv.includes('--collect-release')
    ? await collectTauriReleaseDmg()
    : await packageTauriDmg();
  console.log(`[tauri-package] DMG: ${result.dmgPath}`);
  console.log(`[tauri-package] SHA-256: ${result.checksumPath}`);
}
