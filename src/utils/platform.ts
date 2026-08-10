export function detectPlatform(
  runtimeProcess: unknown = Reflect.get(globalThis, 'process'),
  runtimeNavigator: unknown = Reflect.get(globalThis, 'navigator')
): string {
  const processPlatform = readStringProperty(runtimeProcess, 'platform');
  if (processPlatform) return processPlatform;

  const browserPlatform = (
    readStringProperty(
      readObjectProperty(runtimeNavigator, 'userAgentData'),
      'platform'
    ) ||
    readStringProperty(runtimeNavigator, 'platform') ||
    readStringProperty(runtimeNavigator, 'userAgent') ||
    ''
  ).toLowerCase();
  if (browserPlatform.includes('mac')) return 'darwin';
  if (browserPlatform.includes('win')) return 'win32';
  if (browserPlatform.includes('linux')) return 'linux';
  return 'unknown';
}

function readObjectProperty(value: unknown, key: string): unknown {
  return typeof value === 'object' && value !== null
    ? Reflect.get(value, key)
    : undefined;
}

function readStringProperty(value: unknown, key: string): string | undefined {
  const property = readObjectProperty(value, key);
  return typeof property === 'string' ? property : undefined;
}

export const platform = detectPlatform();
export const isWindows = platform === 'win32';
export const isMac = platform === 'darwin';
export const isLinux = platform === 'linux';
