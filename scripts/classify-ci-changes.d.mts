export interface CiChangeClassification {
  docsOnly: boolean;
  rust: boolean;
  tuiOnly: boolean;
}

export function classifyChangedFiles(files: string[]): CiChangeClassification;

export function changedFiles(options?: {
  baseSha?: string | undefined;
  headSha?: string | undefined;
  cwd?: string | undefined;
  run?: (baseSha: string, headSha: string, cwd: string) => string[] | null;
}): string[] | null;

export function classify(
  env: Record<string, string | undefined>
): CiChangeClassification;

export function envWithoutGitBindings(
  env?: Record<string, string | undefined>
): Record<string, string | undefined>;
