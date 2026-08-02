export function detectPlatform(
  runtimeProcess = globalThis.process,
  runtimeNavigator = globalThis.navigator
) {
  if (runtimeProcess?.platform) return runtimeProcess.platform;

  const browserPlatform = (
    runtimeNavigator?.userAgentData?.platform ||
    runtimeNavigator?.platform ||
    runtimeNavigator?.userAgent ||
    ''
  ).toLowerCase();
  if (browserPlatform.includes('mac')) return 'darwin';
  if (browserPlatform.includes('win')) return 'win32';
  if (browserPlatform.includes('linux')) return 'linux';
  return 'unknown';
}

export const platform = detectPlatform();
export const isWindows = platform === 'win32';
export const isMac = platform === 'darwin';
export const isLinux = platform === 'linux';
export const isDevelopment = process.env.NODE_ENV === 'development';

// macOS 也开托盘：菜单栏要显示封面 + 歌名/歌词
export const isCreateTray = isWindows || isLinux || isMac || isDevelopment;
export const isCreateMpris = isLinux;
