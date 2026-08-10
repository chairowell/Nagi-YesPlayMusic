import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const projectRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '..'
);
const defaultArchivePath = path.join(
  projectRoot,
  'src-tauri/target/aarch64-apple-darwin/release/bundle/macos/YesPlayMusic.app.tar.gz'
);

function run(command, args) {
  const result = Bun.spawnSync([command, ...args], {
    stdout: 'inherit',
    stderr: 'inherit',
  });
  if (result.exitCode !== 0) {
    throw new Error(`${command} failed with exit code ${result.exitCode}`);
  }
}

export async function verifyMacOSUpdaterArtifact(
  archivePath = defaultArchivePath
) {
  const extractionDir = await mkdtemp(
    path.join(tmpdir(), 'yesplaymusic-updater-')
  );
  try {
    run('tar', ['-xzf', archivePath, '-C', extractionDir]);
    const appPath = path.join(extractionDir, 'YesPlayMusic.app');
    run('codesign', ['--verify', '--deep', '--strict', '--verbose=2', appPath]);
    return appPath;
  } finally {
    await rm(extractionDir, { recursive: true, force: true });
  }
}

if (import.meta.main) {
  try {
    await verifyMacOSUpdaterArtifact(process.argv[2]);
    console.log('[tauri-updater] macOS archive signature verified');
  } catch (error) {
    console.error(`[tauri-updater] ${error.message}`);
    process.exit(1);
  }
}
