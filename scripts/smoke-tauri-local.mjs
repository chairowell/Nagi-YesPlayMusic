#!/usr/bin/env bun
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseProcessTable } from './lib/processMetrics.mjs';

const projectRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '..'
);
const executable = path.join(
  projectRoot,
  'src-tauri/target/aarch64-apple-darwin/release/bundle/macos/YesPlayMusic.app/Contents/MacOS/yesplaymusic-tauri'
);
const baseUrl = 'http://127.0.0.1:28232';
const sleep = milliseconds =>
  new Promise(resolve => setTimeout(resolve, milliseconds));
const includeWebview = !process.argv.includes('--core-only');
let activeTauriProcess = null;

function readProcessTable() {
  const result = Bun.spawnSync([
    'ps',
    '-axo',
    'pid=,ppid=,rss=,%cpu=,command=',
  ]);
  if (result.exitCode !== 0) {
    throw new Error(new TextDecoder().decode(result.stderr).trim());
  }
  return parseProcessTable(new TextDecoder().decode(result.stdout));
}

function forwardOutput(stream, target, onText = () => {}) {
  return (async () => {
    const reader = stream.getReader();
    const decoder = new TextDecoder();
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      target.write(value);
      onText(decoder.decode(value, { stream: true }));
    }
  })();
}

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
  const beforePids = new Set(readProcessTable().map(process => process.pid));
  let resolveWebviewReady;
  let observedOutput = '';
  const webviewReady = new Promise(resolve => {
    resolveWebviewReady = resolve;
  });
  const tauriProcess = Bun.spawn(
    [
      executable,
      includeWebview ? '--webview-smoke-test' : '--smoke-test',
    ],
    {
      cwd: projectRoot,
      stdout: 'pipe',
      stderr: 'pipe',
    }
  );
  activeTauriProcess = tauriProcess;
  const stdoutTask = forwardOutput(
    tauriProcess.stdout,
    process.stdout,
    text => {
      observedOutput = `${observedOutput}${text}`.slice(-4_096);
      if (observedOutput.includes('[tauri] webview-ready:')) {
        resolveWebviewReady();
      }
    }
  );
  const stderrTask = forwardOutput(tauriProcess.stderr, process.stderr);

  const loginStatus = await waitForReady();
  const home = await fetch(baseUrl).then(response => response.text());
  if (!home.includes('<div id="app"></div>')) {
    throw new Error('Tauri 首页没有返回 Vue 挂载点');
  }
  if (loginStatus?.data?.code !== 200) {
    throw new Error('Tauri 同源 API 没有返回 200');
  }

  let webkitPids = [];
  if (includeWebview) {
    const loaded = await Promise.race([
      webviewReady.then(() => true),
      sleep(10_000).then(() => false),
    ]);
    if (!loaded) throw new Error('隐藏 WebView 没有在 10 秒内完成页面加载');
    await sleep(3_000);
    webkitPids = readProcessTable()
      .filter(
        process =>
          !beforePids.has(process.pid) &&
          process.command.includes('com.apple.WebKit.')
      )
      .map(process => process.pid);
    if (webkitPids.length === 0) {
      throw new Error('没有识别到本次启动创建的 WebKit XPC 进程');
    }
  }

  const metricsArgs = [
    'bun',
    'scripts/measure-process-tree.mjs',
    '--pid',
    String(tauriProcess.pid),
    '--duration',
    includeWebview ? '8' : '5',
    '--interval',
    '1',
    '--label',
    includeWebview ? 'tauri-hidden-webview-smoke' : 'tauri-core-smoke',
  ];
  if (webkitPids.length) {
    metricsArgs.push('--include-pids', webkitPids.join(','));
  }
  const metricsProcess = Bun.spawn(
    metricsArgs,
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
  activeTauriProcess = null;

  await Promise.all([stdoutTask, stderrTask]);
  await assertStopped();
  console.log(metricsOutput.trim());
  console.log(
    `[tauri-smoke] UI、API、${
      includeWebview ? '隐藏 WebView、' : ''
    }内存采样和进程回收全部通过`
  );
}

main().catch(error => {
  if (activeTauriProcess?.exitCode === null) {
    activeTauriProcess.kill();
  }
  console.error(`[tauri-smoke] ${error.message}`);
  process.exit(1);
});
