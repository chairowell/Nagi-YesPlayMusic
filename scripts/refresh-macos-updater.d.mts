export const defaultMacOSAppPath: string;

export function refreshMacOSUpdaterArtifact(appPath?: string): Promise<{
  archivePath: string;
  signaturePath: string;
}>;
