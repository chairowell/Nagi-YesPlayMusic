export interface TauriVersionFields {
  packageVersion: string;
  tauriVersion: string;
  cargoVersion: string | undefined;
  tag?: string;
}

export function validateTauriVersions(fields: TauriVersionFields): string;

export function verifyTauriVersions(tag?: string): Promise<string>;
