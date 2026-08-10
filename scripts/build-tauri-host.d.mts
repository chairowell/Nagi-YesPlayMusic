export interface TauriHostBuildOptions {
  platform?: NodeJS.Platform;
  arch?: string;
}

export type TauriHostBuildPlan =
  | {
      script: 'build:tauri:macos';
      artifact: 'macOS Apple Silicon app';
    }
  | {
      script: 'build:tauri:windows';
      artifact: 'Windows x64 NSIS setup.exe';
    }
  | {
      script: 'build:tauri:linux';
      artifact: 'Linux x64 AppImage 和 deb';
    };

export function tauriHostBuildPlan(
  options?: TauriHostBuildOptions
): TauriHostBuildPlan;

export function buildTauriForHost(
  options?: TauriHostBuildOptions
): TauriHostBuildPlan;
