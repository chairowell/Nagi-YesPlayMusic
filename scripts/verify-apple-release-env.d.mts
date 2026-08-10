export const REQUIRED_APPLE_RELEASE_ENV: readonly [
  'APPLE_CERTIFICATE',
  'APPLE_CERTIFICATE_PASSWORD',
  'APPLE_SIGNING_IDENTITY',
  'APPLE_ID',
  'APPLE_PASSWORD',
  'APPLE_TEAM_ID',
  'KEYCHAIN_PASSWORD'
];

export function verifyAppleReleaseEnvironment(
  environment?: Readonly<Record<string, string | undefined>>
): true;
