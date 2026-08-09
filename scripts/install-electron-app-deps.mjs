#!/usr/bin/env node
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const projectRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '..'
);
const cli = path.join(
  projectRoot,
  'node_modules',
  'electron-builder',
  'install-app-deps.js'
);
const environment = { ...process.env, npm_execpath: '' };
const result = spawnSync(process.execPath, [cli], {
  cwd: projectRoot,
  env: environment,
  stdio: 'inherit',
});

if (result.error) throw result.error;
process.exit(result.status ?? 1);
