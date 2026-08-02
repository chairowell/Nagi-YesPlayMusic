export const REQUIRED_APPLE_RELEASE_ENV = [
  'APPLE_CERTIFICATE',
  'APPLE_CERTIFICATE_PASSWORD',
  'APPLE_SIGNING_IDENTITY',
  'APPLE_ID',
  'APPLE_PASSWORD',
  'APPLE_TEAM_ID',
  'KEYCHAIN_PASSWORD',
];

export function verifyAppleReleaseEnvironment(environment = process.env) {
  const missing = REQUIRED_APPLE_RELEASE_ENV.filter(
    name => typeof environment[name] !== 'string' || !environment[name].trim()
  );
  if (missing.length) {
    throw new Error(`缺少 Apple 发版密钥：${missing.join(', ')}`);
  }
  return true;
}

if (import.meta.main) {
  try {
    verifyAppleReleaseEnvironment();
    console.log('[tauri-release] Apple 签名与公证密钥已配置');
  } catch (error) {
    console.error(`[tauri-release] ${error.message}`);
    process.exit(1);
  }
}
