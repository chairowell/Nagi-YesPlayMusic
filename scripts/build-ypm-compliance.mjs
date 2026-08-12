#!/usr/bin/env bun
import path from 'node:path';
import {
  buildCompliance,
  complianceSourceArchiveName,
  defaultProjectRoot,
  YPM_TARGET_SPEC,
} from './build-sidecar-compliance.mjs';

export const defaultYpmComplianceOutput = path.join(
  defaultProjectRoot,
  'src-tauri',
  'generated',
  'ypm-compliance'
);
export const defaultYpmCompleteSourceOutput = path.join(
  defaultProjectRoot,
  'src-tauri',
  'generated',
  'ypm-complete-source'
);

export function ypmSourceArchiveName(version) {
  return complianceSourceArchiveName(YPM_TARGET_SPEC, version);
}

export function buildYpmCompliance(options = {}) {
  return buildCompliance(YPM_TARGET_SPEC, options);
}

async function main() {
  const arguments_ = process.argv.slice(2);
  const noticesOnly = arguments_.includes('--notices-only');
  const unknownArguments = arguments_.filter(
    argument => argument !== '--notices-only'
  );
  if (unknownArguments.length > 0) {
    throw new Error(
      `Usage: bun scripts/build-ypm-compliance.mjs [--notices-only] (unexpected: ${unknownArguments.join(
        ' '
      )})`
    );
  }
  const result = await buildYpmCompliance({ noticesOnly });
  console.log(
    `[ypm-compliance] ${
      result.dependencyCount
    } complete dependency sources; bundled notices -> ${
      result.outputDirectory
    }${
      result.completeSourceDirectory
        ? `; source kit -> ${result.completeSourceDirectory}`
        : ''
    }`
  );
}

if (import.meta.main) {
  main().catch(error => {
    const message = error instanceof Error ? error.message : String(error);
    console.error(`[ypm-compliance] ${message}`);
    process.exitCode = 1;
  });
}
