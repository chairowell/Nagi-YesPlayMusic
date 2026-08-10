export interface PackageTauriDmgOptions {
  appPath?: string;
  outputDir?: string;
}

export interface CollectTauriReleaseDmgOptions extends PackageTauriDmgOptions {
  sourcePath?: string;
}

export interface TauriDmgResult {
  dmgPath: string;
  checksumPath: string;
}

export const defaultTauriAppPath: string;

export function tauriDmgName(version: string): string;

export function tauriBundledDmgPath(version: string): string;

export function packageTauriDmg(
  options?: PackageTauriDmgOptions
): Promise<TauriDmgResult>;

export function collectTauriReleaseDmg(
  options?: CollectTauriReleaseDmgOptions
): Promise<TauriDmgResult>;
