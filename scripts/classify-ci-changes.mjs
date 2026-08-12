#!/usr/bin/env bun
// Decides how much of the CI matrix a change needs. Prints GITHUB_OUTPUT lines.
//
// Fail open: anything unresolvable (tag, dispatch, missing base, force push)
// runs the full matrix. A wrong "skip" hides a real break; a wrong "run" only
// costs minutes.
import { spawnSync } from 'node:child_process';

// Touching these can change Rust behaviour, so cargo test/clippy/fmt must run.
const RUST_PATTERNS = [
  /^src-tauri\//,
  /^rust-toolchain/,
  /^\.github\/workflows\//,
  /^scripts\//,
  /^package\.json$/,
  /^bun\.lock$/,
];

// Everything else is documentation: no packaging, no Rust, gates still run.
const DOCS_PATTERNS = [
  /^docs\//,
  /^images\//,
  /^legal\//,
  /\.md$/,
  /^LICENSE$/,
  /^\.github\/(ISSUE_TEMPLATE|PULL_REQUEST_TEMPLATE)/,
];

export function classifyChangedFiles(files) {
  if (files.length === 0) return { docsOnly: false, rust: true };
  const docsOnly = files.every(file =>
    DOCS_PATTERNS.some(pattern => pattern.test(file))
  );
  const rust = files.some(file =>
    RUST_PATTERNS.some(pattern => pattern.test(file))
  );
  return { docsOnly, rust: docsOnly ? false : rust };
}

export function changedFiles({ baseSha, headSha, run = gitDiff } = {}) {
  const zero = /^0*$/;
  if (!baseSha || !headSha || zero.test(baseSha)) return null;
  return run(baseSha, headSha);
}

function gitDiff(baseSha, headSha) {
  const result = spawnSync(
    'git',
    ['diff', '--name-only', `${baseSha}...${headSha}`],
    { encoding: 'utf8' }
  );
  if (result.status !== 0) return null;
  return result.stdout.split('\n').filter(Boolean);
}

export function classify(env) {
  if (env.FORCE_FULL === 'true') return { docsOnly: false, rust: true };
  const files = changedFiles({ baseSha: env.BASE_SHA, headSha: env.HEAD_SHA });
  if (!files) return { docsOnly: false, rust: true };
  return classifyChangedFiles(files);
}

if (import.meta.main) {
  const { docsOnly, rust } = classify(process.env);
  console.log(`docs-only=${docsOnly}`);
  console.log(`rust=${rust}`);
}
