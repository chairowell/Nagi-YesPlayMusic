#!/usr/bin/env bun
import {
  collectProcessTree,
  parseMetricsArgs,
  parseProcessTable,
  summarizeSamples,
} from './lib/processMetrics.mjs';

const sleep = milliseconds =>
  new Promise(resolve => setTimeout(resolve, milliseconds));

function takeSample(options) {
  const result = Bun.spawnSync([
    'ps',
    '-axo',
    'pid=,ppid=,rss=,%cpu=,command=',
  ]);
  if (result.exitCode !== 0) {
    throw new Error(new TextDecoder().decode(result.stderr).trim());
  }

  const processes = parseProcessTable(new TextDecoder().decode(result.stdout));
  const tree = collectProcessTree(processes, options.pid, options.includePids);
  if (!tree.some(process => process.pid === options.pid)) {
    throw new Error(`根进程 ${options.pid} 不存在`);
  }

  return {
    at: new Date().toISOString(),
    rssMiB: tree.reduce((total, process) => total + process.rssKiB, 0) / 1024,
    cpuPercent: tree.reduce((total, process) => total + process.cpuPercent, 0),
    processes: tree,
  };
}

async function main() {
  const options = parseMetricsArgs(process.argv.slice(2));
  const sampleCount = Math.max(
    1,
    Math.ceil(options.durationSeconds / options.intervalSeconds)
  );
  const samples = [];

  for (let index = 0; index < sampleCount; index += 1) {
    samples.push(takeSample(options));
    if (index + 1 < sampleCount) {
      await sleep(options.intervalSeconds * 1000);
    }
  }

  const lastProcesses = samples.at(-1).processes.map(process => ({
    pid: process.pid,
    ppid: process.ppid,
    rssMiB: Math.round((process.rssKiB / 1024) * 100) / 100,
    cpuPercent: process.cpuPercent,
    command: process.command,
  }));

  console.log(
    JSON.stringify(
      {
        label: options.label,
        rootPid: options.pid,
        includedPids: options.includePids,
        durationSeconds: options.durationSeconds,
        intervalSeconds: options.intervalSeconds,
        ...summarizeSamples(samples),
        lastProcesses,
      },
      null,
      2
    )
  );
}

main().catch(error => {
  console.error(error.message);
  process.exit(1);
});
