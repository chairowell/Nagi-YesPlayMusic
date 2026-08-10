export interface TauriSmokeExecutableOptions {
  platform?: NodeJS.Platform;
  arch?: string;
  root?: string;
}

export function resolveTauriSmokeExecutable(
  options?: TauriSmokeExecutableOptions
): string;
