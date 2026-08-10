import { expect, test } from 'bun:test';
import path from 'node:path';
import { createLocalSigningSteps } from '../scripts/tauriSigning.mjs';

test('Tauri 本地包先修复 Bun 签名槽，再签主程序和 app', () => {
  const appPath = path.join(path.sep, 'tmp', 'YesPlayMusic.app');
  const steps = createLocalSigningSteps(appPath);

  expect(steps.map(step => step.label)).toEqual([
    '清除 Bun sidecar 的旧签名槽',
    '签名 Bun sidecar',
    '签名 Tauri 主程序',
    '签名 app bundle',
    '严格校验完整 app bundle',
  ]);
  expect(steps[1].args).toContain(
    path.join(appPath, 'Contents', 'MacOS', 'yesplaymusic-sidecar')
  );
  expect(steps[4].args).toContain('--deep');
  expect(steps[4].args).toContain('--strict');
});
