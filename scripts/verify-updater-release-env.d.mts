export const REQUIRED_UPDATER_RELEASE_ENV: readonly string[];

export function verifyUpdaterReleaseEnvironment(
  environment?: Record<string, string | undefined>
): true;
