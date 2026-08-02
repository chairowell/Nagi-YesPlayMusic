#!/usr/bin/env bun
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const projectRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '..'
);
const executable = path.join(
  projectRoot,
  'src-tauri/target/aarch64-apple-darwin/release/bundle/macos/YesPlayMusic Tauri.app/Contents/MacOS/yesplaymusic-tauri'
);
const baseUrl = 'http://127.0.0.1:28232';
const sleep = milliseconds =>
  new Promise(resolve => setTimeout(resolve, milliseconds));

async function waitForReady(timeoutMs = 8_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`${baseUrl}/api/login/status`);
      if (response.ok) return response.json();
    } catch (_) {
      // sidecar 会先加载 1,077 个模块，ready 前的连接失败属于预期状态。
    }
    await sleep(100);
  }
  throw new Error('Tauri sidecar 未在 8 秒内进入 ready');
}

async function assertStopped() {
  for (let attempt = 0; attempt < 20; attempt += 1) {
    try {
      await fetch(baseUrl);
    } catch (_) {
      return;
    }
    await sleep(100);
  }
  throw new Error('Tauri 退出后 28232 端口仍可访问');
}

async function main() {
  const tauriProcess = Bun.spawn([executable, '--smoke-test'], {
    cwd: projectRoot,
    stdout: 'inherit',
    stderr: 'inherit',
  });

  const loginStatus = await waitForReady();
  const home = await fetch(baseUrl).then(response => response.text());
  if (!home.includes('<div id="app"></div>')) {
    throw new Error('Tauri 首页没有返回 Vue 挂载点');
  }
  if (loginStatus?.data?.code !== 200) {
    throw new Error('Tauri 同源 API 没有返回 200');
  }

  const metricsProcess = Bun.spawn(
    [
      'bun',
      'scripts/measure-process-tree.mjs',
      '--pid',
      String(tauriProcess.pid),
      '--duration',
      '5',
      '--interval',
      '1',
      '--label',
      'tauri-headless-smoke',
    ],
    { cwd: projectRoot, stdout: 'pipe', stderr: 'pipe' }
  );
  const [metricsOutput, metricsError, metricsExitCode] = await Promise.all([
    new Response(metricsProcess.stdout).text(),
    new Response(metricsProcess.stderr).text(),
    metricsProcess.exited,
  ]);
  if (metricsExitCode !== 0) throw new Error(metricsError.trim());

  const exitCode = await Promise.race([
    tauriProcess.exited,
    sleep(15_000).then(() => null),
  ]);
  if (exitCode === null) {
    tauriProcess.kill();
    throw new Error('Tauri smoke 进程没有按时自动退出');
  }
  if (exitCode !== 0) throw new Error(`Tauri smoke 退出码为 ${exitCode}`);

  await assertStopped();
  console.log(metricsOutput.trim());
  console.log('[tauri-smoke] UI、API、内存采样和进程回收全部通过');
}

main().catch(error => {
  console.error(`[tauri-smoke] ${error.message}`);
  process.exit(1);
});
