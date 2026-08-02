function parsePositiveNumber(value, flag) {
  const number = Number(value);
  if (!Number.isFinite(number) || number <= 0) {
    throw new Error(`${flag} 必须是正数`);
  }
  return number;
}

export function parseMetricsArgs(args) {
  const options = {
    pid: null,
    includePids: [],
    durationSeconds: 60,
    intervalSeconds: 1,
    label: 'unnamed',
  };

  for (let index = 0; index < args.length; index += 1) {
    const flag = args[index];
    const value = args[index + 1];
    if (!value || value.startsWith('--')) throw new Error(`${flag} 缺少参数`);

    switch (flag) {
      case '--pid':
        options.pid = parsePositiveNumber(value, flag);
        if (!Number.isInteger(options.pid)) throw new Error('--pid 必须是整数');
        break;
      case '--duration':
        options.durationSeconds = parsePositiveNumber(value, flag);
        break;
      case '--include-pids':
        options.includePids = value.split(',').map(pid => {
          const parsed = parsePositiveNumber(pid, flag);
          if (!Number.isInteger(parsed)) {
            throw new Error(`${flag} 必须是整数列表`);
          }
          return parsed;
        });
        break;
      case '--interval':
        options.intervalSeconds = parsePositiveNumber(value, flag);
        break;
      case '--label':
        options.label = value;
        break;
      default:
        throw new Error(`未知参数：${flag}`);
    }
    index += 1;
  }

  if (!options.pid) throw new Error('必须通过 --pid 指定根进程');
  return options;
}

export function parseProcessTable(text) {
  return text
    .split('\n')
    .map(line => {
      const match = line.match(/^\s*(\d+)\s+(\d+)\s+(\d+)\s+([\d.]+)\s+(.*)$/);
      if (!match) return null;
      return {
        pid: Number(match[1]),
        ppid: Number(match[2]),
        rssKiB: Number(match[3]),
        cpuPercent: Number(match[4]),
        command: match[5],
      };
    })
    .filter(Boolean);
}

export function collectProcessTree(processes, rootPid, includePids = []) {
  const selectedPids = new Set([rootPid, ...includePids]);
  let changed = true;

  while (changed) {
    changed = false;
    for (const process of processes) {
      if (selectedPids.has(process.ppid) && !selectedPids.has(process.pid)) {
        selectedPids.add(process.pid);
        changed = true;
      }
    }
  }

  return processes.filter(process => selectedPids.has(process.pid));
}

function percentile(values, ratio) {
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.max(0, Math.ceil(sorted.length * ratio) - 1)];
}

function round(value) {
  return Math.round(value * 100) / 100;
}

export function summarizeSamples(samples) {
  if (samples.length === 0) throw new Error('没有可汇总的采样');

  const rssValues = samples.map(sample => sample.rssMiB);
  const cpuValues = samples.map(sample => sample.cpuPercent);
  const average = values =>
    values.reduce((total, value) => total + value, 0) / values.length;

  return {
    samples: samples.length,
    rssMiB: {
      mean: round(average(rssValues)),
      p95: round(percentile(rssValues, 0.95)),
      max: round(Math.max(...rssValues)),
    },
    cpuPercent: {
      mean: round(average(cpuValues)),
      p95: round(percentile(cpuValues, 0.95)),
      max: round(Math.max(...cpuValues)),
    },
  };
}
