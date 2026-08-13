export interface TauriVersionFields {
  packageVersion: string;
  tauriVersion: string;
  cargoVersion: string | undefined;
  coreVersion: string | undefined;
  sidecarVersion: string | undefined;
  lockCargoVersion: string | undefined;
  lockCoreVersion: string | undefined;
  lockSidecarVersion: string | undefined;
  tag?: string;
}

export function readUniqueCargoLockPackageVersion(
  cargoLock: string,
  packageName: string
): string;

export function validateTauriVersions(fields: TauriVersionFields): string;

export function verifyTauriVersions(tag?: string): Promise<string>;
