import { access, rm } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { verifyMacOSUpdaterArtifact } from './verify-macos-updater.mjs';

const projectRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '..'
);

export const defaultMacOSAppPath = path.join(
  projectRoot,
  'src-tauri/target/aarch64-apple-darwin/release/bundle/macos/YesPlayMusic.app'
);

function run(command, args, environment = process.env) {
  const result = Bun.spawnSync([command, ...args], {
    cwd: projectRoot,
    env: environment,
    stdout: 'inherit',
    stderr: 'inherit',
  });
  if (result.exitCode !== 0) {
    throw new Error(`${command} failed with exit code ${result.exitCode}`);
  }
}

export async function refreshMacOSUpdaterArtifact(
  appPath = defaultMacOSAppPath
) {
  await access(appPath);
  const parent = path.dirname(appPath);
  const archivePath = `${appPath}.tar.gz`;
  const signaturePath = `${archivePath}.sig`;
  await rm(archivePath, { force: true });
  await rm(signaturePath, { force: true });
  run('tar', ['-czf', archivePath, '-C', parent, path.basename(appPath)], {
    ...process.env,
    COPYFILE_DISABLE: '1',
  });
  run(process.execPath, ['tauri', 'signer', 'sign', archivePath]);
  await access(signaturePath);
  await verifyMacOSUpdaterArtifact(archivePath);
  return { archivePath, signaturePath };
}

if (import.meta.main) {
  try {
    const result = await refreshMacOSUpdaterArtifact(process.argv[2]);
    console.log(`[tauri-updater] refreshed ${result.archivePath}`);
  } catch (error) {
    console.error(`[tauri-updater] ${error.message}`);
    process.exit(1);
  }
}
