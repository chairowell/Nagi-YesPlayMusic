#!/usr/bin/env bun
// tauri.conf.json's beforeBuildCommand. Every packaging path goes through it —
// mac plain/updater/Developer ID, Windows plain/updater, Linux plain/updater —
// so CI only has to set the flag once per job instead of patching seven steps.
//
// CI prepares these resources before the Rust tests, then packages; without the
// flag the same renderer + Sidecar + compliance work ran twice per platform.
import { spawnSync } from 'node:child_process';
import { readdirSync, statSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { hostTargetTriple } from './build-rust-sidecar.mjs';

const projectRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '..'
);

export const PREPARED_FLAG = 'YPM_TAURI_RESOURCES_PREPARED';

function nonEmptyFile(absolutePath) {
  try {
    return statSync(absolutePath).size > 0;
  } catch {
    return false;
  }
}

function nonEmptyDirectory(absolutePath) {
  try {
    return (
      statSync(absolutePath).isDirectory() &&
      readdirSync(absolutePath).length > 0
    );
  } catch {
    return false;
  }
}

// Each entry is a distinct producer: skipping must not silently drop any of them.
export function preparedResources({
  root = projectRoot,
  targetTriple = process.env.TAURI_ENV_TARGET_TRIPLE || hostTargetTriple(),
} = {}) {
  return [
    {
      label: 'renderer',
      producer: 'bun run build:tauri:renderer',
      files: [path.join(root, 'dist-tauri', 'index.html')],
      directories: [path.join(root, 'dist-tauri', 'assets')],
    },
    {
      label: 'app compliance',
      producer: 'bun run build:tauri:renderer',
      files: [
        path.join(
          root,
          'src-tauri',
          'generated',
          'app-compliance',
          'SHA256SUMS'
        ),
        path.join(
          root,
          'src-tauri',
          'generated',
          'app-compliance',
          'APP-COMPLIANCE-MANIFEST.json'
        ),
      ],
      directories: [],
    },
    {
      label: 'Rust Sidecar',
      producer: 'bun run build:sidecar',
      files: [
        path.join(
          root,
          'src-tauri',
          'binaries',
          `yesplaymusic-sidecar-${targetTriple}`
        ),
      ],
      directories: [],
    },
    {
      label: 'sidecar compliance',
      producer: 'bun run build:sidecar',
      files: [
        path.join(
          root,
          'src-tauri',
          'generated',
          'sidecar-compliance',
          'SHA256SUMS'
        ),
        path.join(
          root,
          'src-tauri',
          'generated',
          'sidecar-compliance',
          'SOURCE-MANIFEST.json'
        ),
      ],
      directories: [],
    },
  ];
}

export function missingPreparedResources(options = {}) {
  const missing = [];
  for (const resource of preparedResources(options)) {
    const absent = [
      ...resource.files.filter(file => !nonEmptyFile(file)),
      ...resource.directories.filter(
        directory => !nonEmptyDirectory(directory)
      ),
    ];
    if (absent.length > 0) missing.push({ ...resource, absent });
  }
  return missing;
}

// The flag is only honoured inside GitHub Actions: a stale local export must
// never turn a developer's `bun run build:tauri` into a silent no-op.
export function shouldSkipPreparation(env = process.env) {
  return env.GITHUB_ACTIONS === 'true' && env[PREPARED_FLAG] === '1';
}

function run(command) {
  const result = spawnSync('bun', command, {
    stdio: 'inherit',
    cwd: projectRoot,
  });
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

if (import.meta.main) {
  if (shouldSkipPreparation()) {
    const missing = missingPreparedResources();
    if (missing.length > 0) {
      console.error(
        `[prepare-tauri-build] ${PREPARED_FLAG}=1 但准备好的产物缺失，拒绝跳过：`
      );
      for (const resource of missing) {
        console.error(`  ${resource.label}（由 ${resource.producer} 生成）`);
        for (const absent of resource.absent) {
          console.error(`    缺 ${path.relative(projectRoot, absent)}`);
        }
      }
      process.exit(1);
    }
    console.log(
      '[prepare-tauri-build] 复用 CI 已准备的 renderer 与 Sidecar 产物'
    );
  } else {
    run(['run', 'build:tauri:renderer']);
    run(['run', 'build:sidecar']);
  }
}
