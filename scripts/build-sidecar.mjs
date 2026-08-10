#!/usr/bin/env bun
import { createHash } from 'node:crypto';
import {
  chmodSync,
  closeSync,
  mkdirSync,
  openSync,
  readSync,
  renameSync,
  rmSync,
  unlinkSync,
  writeFileSync,
  writeSync,
} from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const projectRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '..'
);
const LINUX_TARGET_TRIPLE = 'x86_64-unknown-linux-gnu';
const LINUX_PAYLOAD_NAME = 'yesplaymusic-sidecar-linux.payload';
const PAYLOAD_HEADER = Buffer.from('YPM1');

export const SIDECAR_TARGETS = Object.freeze({
  'aarch64-apple-darwin': {
    bunTarget: 'bun-darwin-arm64',
    extension: '',
  },
  'x86_64-pc-windows-msvc': {
    // Baseline supports older CPUs without AVX2 at a small performance cost.
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
  const usesPayloadWrapper = targetTriple === LINUX_TARGET_TRIPLE;
  const compileOutputPath = usesPayloadWrapper
    ? `${outputPath}.raw`
    : outputPath;
  const payloadPath = usesPayloadWrapper
    ? path.join(path.dirname(outputPath), LINUX_PAYLOAD_NAME)
    : null;
  const args = [
    'build',
    path.join(projectRoot, 'src', 'sidecar.ts'),
    '--compile',
    `--target=${target.bunTarget}`,
    '--outfile',
    compileOutputPath,
  ];
  if (targetTriple.includes('windows')) args.push('--windows-hide-console');
  return {
    targetTriple,
    outputName,
    outputPath,
    compileOutputPath,
    payloadPath,
    usesPayloadWrapper,
    args,
  };
}

function writeAll(fileDescriptor, buffer, length = buffer.length) {
  let offset = 0;
  while (offset < length) {
    offset += writeSync(fileDescriptor, buffer, offset, length - offset);
  }
}

function linuxSidecarWrapper(digest) {
  return `#!/bin/sh
set -eu

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
payload="$script_dir/${LINUX_PAYLOAD_NAME}"
if [ ! -r "$payload" ]; then
  payload="$script_dir/../lib/yesplaymusic/sidecar.payload"
fi
if [ ! -r "$payload" ]; then
  echo "YesPlayMusic sidecar payload is missing" >&2
  exit 1
fi
if [ "$(dd if="$payload" bs=4 count=1 2>/dev/null)" != "YPM1" ]; then
  echo "YesPlayMusic sidecar payload is invalid" >&2
  exit 1
fi

if [ -n "\${XDG_CACHE_HOME:-}" ]; then
  cache_root=$XDG_CACHE_HOME
elif [ -n "\${HOME:-}" ]; then
  cache_root=$HOME/.cache
else
  echo "YesPlayMusic sidecar needs HOME or XDG_CACHE_HOME" >&2
  exit 1
fi
cache_dir="$cache_root/yesplaymusic"
cached="$cache_dir/sidecar-${digest}"
umask 077
mkdir -p "$cache_dir"
find "$cache_dir" -maxdepth 1 -type f -name '.sidecar.*' -mtime +0 -delete 2>/dev/null || true

if [ -x "$cached" ] && [ "$(sha256sum "$cached" | awk '{print $1}')" != "${digest}" ]; then
  rm -f "$cached"
fi

if [ ! -x "$cached" ]; then
  temporary=$(mktemp "$cache_dir/.sidecar.XXXXXX")
  trap 'rm -f "$temporary"' EXIT HUP INT TERM
  tail -c +5 "$payload" > "$temporary"
  if [ "$(sha256sum "$temporary" | awk '{print $1}')" != "${digest}" ]; then
    echo "YesPlayMusic sidecar payload checksum failed" >&2
    exit 1
  fi
  chmod 700 "$temporary"
  mv -f "$temporary" "$cached"
  trap - EXIT HUP INT TERM
fi

exec "$cached" "$@"
`;
}

export function writeLinuxSidecarBundle({
  compileOutputPath,
  outputPath,
  payloadPath,
}) {
  const temporaryPayload = `${payloadPath}.tmp-${process.pid}`;
  const source = openSync(compileOutputPath, 'r');
  const destination = openSync(temporaryPayload, 'w', 0o600);
  const buffer = Buffer.allocUnsafe(1024 * 1024);
  const hash = createHash('sha256');
  let completed = false;
  try {
    writeAll(destination, PAYLOAD_HEADER);
    while (true) {
      const length = readSync(source, buffer, 0, buffer.length, null);
      if (length === 0) break;
      hash.update(buffer.subarray(0, length));
      writeAll(destination, buffer, length);
    }
    completed = true;
  } finally {
    closeSync(source);
    closeSync(destination);
    if (!completed) rmSync(temporaryPayload, { force: true });
  }

  const digest = hash.digest('hex');
  renameSync(temporaryPayload, payloadPath);
  chmodSync(payloadPath, 0o644);
  writeFileSync(outputPath, linuxSidecarWrapper(digest), { mode: 0o755 });
  chmodSync(outputPath, 0o755);
  unlinkSync(compileOutputPath);
  return { digest };
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
  if (plan.usesPayloadWrapper) {
    writeLinuxSidecarBundle(plan);
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
