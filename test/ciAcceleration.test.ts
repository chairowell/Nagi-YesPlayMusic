import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import {
  classify,
  classifyChangedFiles,
} from '../scripts/classify-ci-changes.mjs';
import {
  PREPARED_FLAG,
  missingPreparedResources,
  preparedResources,
  shouldSkipPreparation,
} from '../scripts/prepare-tauri-build.mjs';

const workflow = readFileSync(
  new URL('../.github/workflows/build.yaml', import.meta.url),
  'utf8'
);
const tauriConfig = JSON.parse(
  readFileSync(new URL('../src-tauri/tauri.conf.json', import.meta.url), 'utf8')
);

const PLATFORM_JOBS = [
  ['  build-tauri-arm64:', '  build-tauri-windows-x64:'],
  ['  build-tauri-windows-x64:', '  build-tauri-linux-x64:'],
  ['  build-tauri-linux-x64:', '  draft-release:'],
] as const;

function jobBody(start: string, end: string): string {
  return workflow.slice(workflow.indexOf(start), workflow.indexOf(end));
}

async function prepareFixture(): Promise<string> {
  const root = await mkdtemp(path.join(tmpdir(), 'ypm-prepared-'));
  const triple = 'aarch64-apple-darwin';
  for (const resource of preparedResources({ root, targetTriple: triple })) {
    for (const file of resource.files) {
      await mkdir(path.dirname(file), { recursive: true });
      await writeFile(file, 'x');
    }
    for (const directory of resource.directories) {
      await mkdir(directory, { recursive: true });
      await writeFile(path.join(directory, 'asset'), 'x');
    }
  }
  return root;
}

test('七条打包路径统一走同一个 beforeBuild wrapper', () => {
  expect(tauriConfig.build.beforeBuildCommand).toBe(
    'bun scripts/prepare-tauri-build.mjs'
  );
  // Every packaging entry point ends up in `tauri build`, so the wrapper is the
  // single place that has to know about the prepared flag.
  const packagingScripts = JSON.parse(
    readFileSync(new URL('../package.json', import.meta.url), 'utf8')
  ).scripts;
  for (const script of [
    'build:tauri',
    'build:tauri:macos:updater',
    'build:tauri:release',
    'build:tauri:windows',
    'build:tauri:windows:updater',
    'build:tauri:linux',
    'build:tauri:linux:updater',
  ]) {
    expect(packagingScripts[script]).toBeTruthy();
  }
});

test('没有 flag 时正常构建，不跳过任何准备工作', () => {
  expect(shouldSkipPreparation({})).toBe(false);
  expect(shouldSkipPreparation({ [PREPARED_FLAG]: '1' })).toBe(false);
  expect(
    shouldSkipPreparation({ GITHUB_ACTIONS: 'true', [PREPARED_FLAG]: '0' })
  ).toBe(false);
});

test('只有 CI 里设了 flag 才跳过', () => {
  expect(
    shouldSkipPreparation({ GITHUB_ACTIONS: 'true', [PREPARED_FLAG]: '1' })
  ).toBe(true);
});

test('产物齐全才认，缺任何一类都报出缺什么', async () => {
  const root = await prepareFixture();
  try {
    expect(
      missingPreparedResources({ root, targetTriple: 'aarch64-apple-darwin' })
    ).toEqual([]);

    await rm(path.join(root, 'src-tauri', 'binaries'), {
      recursive: true,
      force: true,
    });
    const missing = missingPreparedResources({
      root,
      targetTriple: 'aarch64-apple-darwin',
    });
    expect(missing).toHaveLength(1);
    expect(missing[0]?.label).toBe('Rust Sidecar');
    expect(missing[0]?.absent[0]).toContain('yesplaymusic-sidecar');
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test('空文件不算数，避免半截产物骗过检查', async () => {
  const root = await prepareFixture();
  try {
    await writeFile(path.join(root, 'dist-tauri', 'index.html'), '', 'utf8');
    const missing = missingPreparedResources({
      root,
      targetTriple: 'aarch64-apple-darwin',
    });
    expect(missing.map(resource => resource.label)).toEqual(['renderer']);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test('三平台 job 都先准备资源、再打标记，顺序不能反', () => {
  for (const [start, end] of PLATFORM_JOBS) {
    const job = jobBody(start, end);
    const prepareIndex = job.indexOf('bun run build:tauri:renderer');
    const markIndex = job.indexOf(`echo "${PREPARED_FLAG}=1"`);
    const buildIndex = job.indexOf('run: bun run build:tauri');
    expect(prepareIndex).toBeGreaterThan(-1);
    expect(markIndex).toBeGreaterThan(prepareIndex);
    expect(buildIndex).toBeGreaterThan(markIndex);
  }
});

test('三处 rust-cache 都不再 cache-on-failure', () => {
  expect(workflow).not.toContain('cache-on-failure');
  expect(workflow.match(/Swatinem\/rust-cache@v2/g)).toHaveLength(3);
});

test('纯文档改动跳打包，但门禁照跑', () => {
  expect(classifyChangedFiles(['README.md', 'docs/a.md'])).toEqual({
    docsOnly: true,
    rust: false,
  });
  expect(workflow).toContain('docs-gates');
  const docsJob = jobBody('  docs-gates:', '  build-tauri-arm64:');
  expect(docsJob).toContain("if: needs.changes.outputs.docs-only == 'true'");
  expect(docsJob).toContain('run: bun test');
  expect(docsJob).toContain('run: bun run typecheck');
  for (const [start, end] of PLATFORM_JOBS) {
    expect(jobBody(start, end)).toContain(
      "if: needs.changes.outputs.docs-only != 'true'"
    );
  }
});

test('只改渲染层时跳过 Rust 门禁，改 Rust 时全跑', () => {
  expect(classifyChangedFiles(['src/App.vue', 'src/utils/x.ts'])).toEqual({
    docsOnly: false,
    rust: false,
  });
  expect(classifyChangedFiles(['src-tauri/src/main.rs'])).toEqual({
    docsOnly: false,
    rust: true,
  });
  expect(classifyChangedFiles(['scripts/build-tauri-host.mjs']).rust).toBe(
    true
  );
  expect(classifyChangedFiles(['.github/workflows/build.yaml']).rust).toBe(
    true
  );

  for (const [start, end] of PLATFORM_JOBS) {
    const job = jobBody(start, end);
    expect(
      job.match(/if: needs\.changes\.outputs\.rust == 'true'/g)
    ).toHaveLength(5);
    // Preparing resources is packaging work, not a Rust gate: never skipped.
    const prepareBlock = job.slice(
      job.indexOf('- name: Prepare Tauri resources for Rust tests'),
      job.indexOf('- name: Mark Tauri resources as prepared')
    );
    expect(prepareBlock).not.toContain('needs.changes.outputs.rust');
  }
});

test('分不清改动范围时一律全跑', () => {
  expect(classifyChangedFiles([])).toEqual({ docsOnly: false, rust: true });
  expect(classify({ FORCE_FULL: 'true' })).toEqual({
    docsOnly: false,
    rust: true,
  });
  // Force push and first push give an all-zero or missing base.
  expect(classify({ BASE_SHA: '0'.repeat(40), HEAD_SHA: 'abc' })).toEqual({
    docsOnly: false,
    rust: true,
  });
  expect(classify({ HEAD_SHA: 'abc' })).toEqual({
    docsOnly: false,
    rust: true,
  });
});

test('tag 与手动触发强制全跑，classify job 不参与判断', () => {
  const classifyJob = jobBody('  changes:', '  docs-gates:');
  expect(classifyJob).toContain(
    "FORCE_FULL: ${{ (github.ref_type == 'tag' || github.event_name == 'workflow_dispatch') && 'true' || 'false' }}"
  );
  expect(classifyJob).toContain('fetch-depth: 0');
});
