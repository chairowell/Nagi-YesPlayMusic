#!/usr/bin/env bun

export function tauriHostBuildPlan({
  platform = process.platform,
  arch = process.arch,
} = {}) {
  const key = `${platform}-${arch}`;
  const plans = {
    'darwin-arm64': {
      script: 'build:tauri:macos',
      artifact: 'macOS Apple Silicon app',
    },
    'win32-x64': {
      script: 'build:tauri:windows',
      artifact: 'Windows x64 NSIS setup.exe',
    },
    'linux-x64': {
      script: 'build:tauri:linux',
      artifact: 'Linux x64 AppImage 和 deb',
    },
  };
  const plan = plans[key];
  if (!plan) throw new Error(`暂不支持在 ${platform}/${arch} 构建 Tauri 应用`);
  return plan;
}

export function buildTauriForHost(options) {
  const plan = tauriHostBuildPlan(options);
  console.log(`[tauri] building ${plan.artifact}`);
  const result = Bun.spawnSync([process.execPath, 'run', plan.script], {
    stdout: 'inherit',
    stderr: 'inherit',
  });
  if (result.exitCode !== 0) {
    throw new Error(`${plan.script} 失败，退出码 ${result.exitCode}`);
  }
  return plan;
}

if (import.meta.main) {
  try {
    buildTauriForHost();
  } catch (error) {
    console.error(`[tauri] ${error.message}`);
    process.exit(1);
  }
}
