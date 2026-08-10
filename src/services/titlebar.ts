export type CustomTitlebar = 'windows' | 'linux' | null;

export function resolveCustomTitlebar(
  platform: string,
  isDesktop: boolean,
  linuxEnabled: boolean
): CustomTitlebar {
  if (!isDesktop) return null;
  if (platform === 'win32') return 'windows';
  if (platform === 'linux' && linuxEnabled) return 'linux';
  return null;
}
