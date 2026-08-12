import type { CargoMetadata } from './build-sidecar-compliance.mjs';

export interface YpmComplianceBuildOptions {
  projectRoot?: string;
  outputDirectory?: string;
  completeSourceDirectory?: string;
  metadata?: CargoMetadata;
  binaryProvenance?: {
    targetTriple: string;
    fileName: string;
    sha256: string;
    rustMarker?: null;
    machOUuid: string | null;
  };
  skipOfflineRebuild?: boolean;
  noticesOnly?: boolean;
}

export interface YpmComplianceBuildResult {
  outputDirectory: string;
  completeSourceDirectory: string | null;
  dependencyCount: number;
  copyleftSourceCount: number;
}

export const defaultYpmComplianceOutput: string;
export const defaultYpmCompleteSourceOutput: string;

export function ypmSourceArchiveName(version: string): string;

export function buildYpmCompliance(
  options?: YpmComplianceBuildOptions
): Promise<YpmComplianceBuildResult>;
