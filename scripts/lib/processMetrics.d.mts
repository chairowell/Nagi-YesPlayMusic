export interface ProcessMetricsOptions {
  pid: number;
  includePids: number[];
  durationSeconds: number;
  intervalSeconds: number;
  label: string;
}

export interface ProcessTableEntry {
  pid: number;
  ppid: number;
  rssKiB: number;
  cpuPercent: number;
  command: string;
}

export interface ProcessSample {
  rssMiB: number;
  cpuPercent: number;
}

export interface MetricSummary {
  mean: number;
  p95: number;
  max: number;
}

export interface ProcessSamplesSummary {
  samples: number;
  rssMiB: MetricSummary;
  cpuPercent: MetricSummary;
}

export function parseMetricsArgs(
  args: readonly string[]
): ProcessMetricsOptions;

export function parseProcessTable(text: string): ProcessTableEntry[];

export function collectProcessTree(
  processes: readonly ProcessTableEntry[],
  rootPid: number,
  includePids?: readonly number[]
): ProcessTableEntry[];

export function summarizeSamples(
  samples: readonly ProcessSample[]
): ProcessSamplesSummary;
