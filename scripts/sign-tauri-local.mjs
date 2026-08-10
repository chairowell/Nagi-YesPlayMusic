#!/usr/bin/env bun
import { defaultTauriAppPath, signLocalTauriBundle } from './tauriSigning.mjs';

const appPath = process.argv[2] || defaultTauriAppPath;

try {
  signLocalTauriBundle(appPath);
  console.log(`[tauri-sign] verified: ${appPath}`);
} catch (error) {
  console.error(`[tauri-sign] ${error.message}`);
  process.exit(1);
}
