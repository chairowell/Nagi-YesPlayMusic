export const isWindows = process.platform === 'win32';
export const isMac = process.platform === 'darwin';
export const isLinux = process.platform === 'linux';
export const isDevelopment = process.env.NODE_ENV === 'development';

// macOS 也开托盘：菜单栏要显示封面 + 歌名/歌词
export const isCreateTray = isWindows || isLinux || isMac || isDevelopment;
export const isCreateMpris = isLinux;
