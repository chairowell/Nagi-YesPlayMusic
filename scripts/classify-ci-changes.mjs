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
  /^src\/sidecar-route-manifest\.json$/,
  /^test\/fixtures\/proxy-ca\.pem$/,
  /^rust-toolchain/,
  /^\.github\/workflows\//,
  /^scripts\//,
  /^package\.json$/,
  /^bun\.lock$/,
];

// These files are documentation only: no packaging, no Rust, cheap gates run.
const DOCS_PATTERNS = [
  /^docs\//,
  /^images\//,
  /^(?:AGENTS|CLAUDE|README)\.md$/,
  /^\.github\/(ISSUE_TEMPLATE|PULL_REQUEST_TEMPLATE)/,
];

// These are known packaging/renderer inputs that do not require Cargo gates.
// Anything outside this allowlist fails open and runs the Rust gates.
const KNOWN_NON_RUST_PATTERNS = [
  ...DOCS_PATTERNS,
  /^legal\//,
  /^LICENSE$/,
  /^src\//,
  /^test\//,
  /^public\//,
  /^build\//,
  /^index\.html$/,
  /^vite\.config\.mjs$/,
  /^tsconfig(?:\.[^.]+)?\.json$/,
  /^\.env\.example$/,
];

function matches(file, patterns) {
  return patterns.some(pattern => pattern.test(file));
}

export function classifyChangedFiles(files) {
  if (files.length === 0) return { docsOnly: false, rust: true };
  const docsOnly = files.every(file => matches(file, DOCS_PATTERNS));
  const rust = files.some(
    file =>
      matches(file, RUST_PATTERNS) || !matches(file, KNOWN_NON_RUST_PATTERNS)
  );
  return { docsOnly, rust };
}

export function changedFiles({
  baseSha,
  headSha,
  cwd = process.cwd(),
  run = gitDiff,
} = {}) {
  const zero = /^0*$/;
  if (!baseSha || !headSha || zero.test(baseSha)) return null;
  return run(baseSha, headSha, cwd);
}

// Strip hook-injected repo bindings (a pre-commit hook exports GIT_DIR,
// absolute inside a worktree) so `cwd` alone decides which repo git sees.
export function envWithoutGitBindings(env = process.env) {
  const scrubbed = { ...env };
  for (const key of [
    'GIT_DIR',
    'GIT_WORK_TREE',
    'GIT_INDEX_FILE',
    'GIT_OBJECT_DIRECTORY',
    'GIT_COMMON_DIR',
  ]) {
    delete scrubbed[key];
  }
  return scrubbed;
}

function gitDiff(baseSha, headSha, cwd) {
  const result = spawnSync(
    'git',
    ['diff', '--no-renames', '--name-only', '-z', `${baseSha}...${headSha}`],
    { cwd, encoding: 'utf8', env: envWithoutGitBindings() }
  );
  if (result.status !== 0) return null;
  return result.stdout.split('\0').filter(Boolean);
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
