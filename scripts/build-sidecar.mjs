#!/usr/bin/env bun
import { mkdirSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const projectRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '..'
);

export const SIDECAR_TARGETS = Object.freeze({
  'aarch64-apple-darwin': {
    bunTarget: 'bun-darwin-arm64',
    extension: '',
  },
  'x86_64-pc-windows-msvc': {
    // baseline 能覆盖没有 AVX2 的旧电脑，代价只是 Sidecar 略慢。
    bunTarget: 'bun-windows-x64-baseline',
    extension: '.exe',
  },
  'x86_64-unknown-linux-gnu': {
    bunTarget: 'bun-linux-x64-baseline',
    extension: '',
  },
});

export function hostTargetTriple({
  platform = process.platform,
  arch = process.arch,
} = {}) {
  const key = `${platform}-${arch}`;
  const targets = {
    'darwin-arm64': 'aarch64-apple-darwin',
    'win32-x64': 'x86_64-pc-windows-msvc',
    'linux-x64': 'x86_64-unknown-linux-gnu',
  };
  const targetTriple = targets[key];
  if (!targetTriple) {
    throw new Error(`暂不支持在 ${platform}/${arch} 构建 Tauri Sidecar`);
  }
  return targetTriple;
}

export function sidecarBuildPlan({
  targetTriple = process.env.TAURI_ENV_TARGET_TRIPLE || hostTargetTriple(),
} = {}) {
  const target = SIDECAR_TARGETS[targetTriple];
  if (!target) {
    throw new Error(`暂不支持 Tauri target：${targetTriple}`);
  }
  const outputName = `yesplaymusic-sidecar-${targetTriple}${target.extension}`;
  const outputPath = path.join(
    projectRoot,
    'src-tauri',
    'binaries',
    outputName
  );
  const args = [
    'build',
    path.join(projectRoot, 'src', 'sidecar.js'),
    '--compile',
    `--target=${target.bunTarget}`,
    '--outfile',
    outputPath,
  ];
  if (targetTriple.includes('windows')) args.push('--windows-hide-console');
  return { targetTriple, outputName, outputPath, args };
}

export function buildSidecar(options) {
  const plan = sidecarBuildPlan(options);
  mkdirSync(path.dirname(plan.outputPath), { recursive: true });
  const result = Bun.spawnSync([process.execPath, ...plan.args], {
    cwd: projectRoot,
    stdout: 'inherit',
    stderr: 'inherit',
  });
  if (result.exitCode !== 0) {
    throw new Error(`Sidecar 构建失败，退出码 ${result.exitCode}`);
  }
  console.log(`[sidecar] built ${plan.outputName}`);
  return plan;
}

if (import.meta.main) {
  try {
    buildSidecar();
  } catch (error) {
    console.error(`[sidecar] ${error.message}`);
    process.exit(1);
  }
}
