export interface CiChangeClassification {
  docsOnly: boolean;
  rust: boolean;
}

export function classifyChangedFiles(files: string[]): CiChangeClassification;

export function changedFiles(options?: {
  baseSha?: string | undefined;
  headSha?: string | undefined;
  run?: (baseSha: string, headSha: string) => string[] | null;
}): string[] | null;

export function classify(
  env: Record<string, string | undefined>
): CiChangeClassification;
