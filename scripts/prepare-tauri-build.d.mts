export declare const PREPARED_FLAG: string;

export interface PreparedResource {
  label: string;
  producer: string;
  files: string[];
  directories: string[];
}

export interface PreparedResourceOptions {
  root?: string;
  targetTriple?: string;
}

export function preparedResources(
  options?: PreparedResourceOptions
): PreparedResource[];

export function missingPreparedResources(
  options?: PreparedResourceOptions
): (PreparedResource & { absent: string[] })[];

export function shouldSkipPreparation(
  env?: Record<string, string | undefined>
): boolean;

export interface PrepareTauriBuildOptions extends PreparedResourceOptions {
  env?: Record<string, string | undefined>;
  runCommand?: (command: string[]) => void;
  log?: (message: string) => void;
}

export function prepareTauriBuild(options?: PrepareTauriBuildOptions): {
  skipped: boolean;
};
