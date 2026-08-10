export interface SigningStep {
  label: string;
  args: string[];
  allowFailure?: boolean;
}

export type LocalSigningSteps = [
  SigningStep & { allowFailure: true },
  SigningStep,
  SigningStep,
  SigningStep,
  SigningStep
];

export interface SigningCommandResult {
  exitCode: number;
  stderr?: Uint8Array<ArrayBufferLike> | ArrayBuffer | null;
}

export const defaultTauriAppPath: string;

export function createLocalSigningSteps(
  appPath?: string,
  entitlements?: string
): LocalSigningSteps;

export function signLocalTauriBundle(
  appPath?: string,
  run?: (args: string[]) => SigningCommandResult
): void;
