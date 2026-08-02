import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const authApiSource = readFileSync(
  new URL('../src/api/auth.js', import.meta.url),
  'utf8'
);

function functionSource(name) {
  const start = authApiSource.indexOf(`export function ${name}`);
  const end = authApiSource.indexOf('\n}', start) + 2;
  return authApiSource.slice(start, end);
}

describe('账号密码请求隐私', () => {
  test.each(['loginWithPhone', 'loginWithEmail'])(
    '%s 把凭据放进 POST body 而不是 URL 查询串',
    name => {
      const source = functionSource(name);
      expect(source).toContain('method: \'post\'');
      expect(source).toContain('data: params');
      expect(source).not.toMatch(/\n\s*params,\s*\n/);
    }
  );
});
