export type SidecarTargetTriple =
  | 'aarch64-apple-darwin'
  | 'x86_64-pc-windows-msvc'
  | 'x86_64-unknown-linux-gnu';

export interface SidecarTarget {
  bunTarget: string;
  extension: string;
}

export interface HostTargetOptions {
  platform?: NodeJS.Platform;
  arch?: string;
}

export interface SidecarBuildOptions {
  targetTriple?: string;
}

export interface SidecarBuildPlan {
  targetTriple: SidecarTargetTriple;
  outputName: string;
  outputPath: string;
  compileOutputPath: string;
  payloadPath: string | null;
  usesPayloadWrapper: boolean;
  args: string[];
}

export interface LinuxSidecarBundleOptions {
  compileOutputPath: string;
  outputPath: string;
  payloadPath: string;
}

export const SIDECAR_TARGETS: Readonly<
  Record<SidecarTargetTriple, Readonly<SidecarTarget>>
>;

export function hostTargetTriple(
  options?: HostTargetOptions
): SidecarTargetTriple;

export function sidecarBuildPlan(
  options?: SidecarBuildOptions
): SidecarBuildPlan;

export function writeLinuxSidecarBundle(options: LinuxSidecarBundleOptions): {
  digest: string;
};

export function buildSidecar(options?: SidecarBuildOptions): SidecarBuildPlan;
