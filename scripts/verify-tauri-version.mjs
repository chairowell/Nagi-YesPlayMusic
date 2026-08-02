import path from 'node:path';
import { fileURLToPath } from 'node:url';

const projectRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '..'
);

export function validateTauriVersions({ packageVersion, tauriVersion, cargoVersion, tag }) {
  const versions = new Set([packageVersion, tauriVersion, cargoVersion]);
  if (versions.size !== 1) {
    throw new Error(
      `版本号不一致：package=${packageVersion}, tauri=${tauriVersion}, cargo=${cargoVersion}`
    );
  }
  if (tag && tag !== `v${packageVersion}`) {
    throw new Error(`tag ${tag} 与应用版本 v${packageVersion} 不一致`);
  }
  return packageVersion;
}

export async function verifyTauriVersions(tag = '') {
  const pkg = await Bun.file(path.join(projectRoot, 'package.json')).json();
  const tauri = await Bun.file(
    path.join(projectRoot, 'src-tauri/tauri.conf.json')
  ).json();
  const cargo = await Bun.file(
    path.join(projectRoot, 'src-tauri/Cargo.toml')
  ).text();
  const cargoVersion = cargo.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
  return validateTauriVersions({
    packageVersion: pkg.version,
    tauriVersion: tauri.version,
    cargoVersion,
    tag,
  });
}

if (import.meta.main) {
  const version = await verifyTauriVersions(process.argv[2] || '');
  console.log(`[tauri-version] ${version}`);
}
