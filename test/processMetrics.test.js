import { describe, expect, test } from 'bun:test';
import {
  collectProcessTree,
  parseMetricsArgs,
  parseProcessTable,
  summarizeSamples,
} from '../scripts/lib/processMetrics.mjs';

describe('性能采样参数', () => {
  test('只接受显式根 PID，避免误采样其他 YesPlayMusic', () => {
    expect(() => parseMetricsArgs([])).toThrow('必须通过 --pid 指定根进程');
    expect(parseMetricsArgs(['--pid', '42', '--duration', '5'])).toEqual({
      pid: 42,
      durationSeconds: 5,
      intervalSeconds: 1,
      label: 'unnamed',
    });
  });
});

describe('进程树性能采样', () => {
  const table = `
  10     1 102400  1.5 /Applications/Example.app/main
  11    10  51200  2.0 helper --renderer
  12    11  25600  0.5 helper --gpu
  99     1 999999 80.0 unrelated
`;

  test('递归收集子进程，不混入无关应用', () => {
    const tree = collectProcessTree(parseProcessTable(table), 10);
    expect(tree.map(process => process.pid)).toEqual([10, 11, 12]);
  });

  test('统一输出均值、P95 和峰值', () => {
    expect(
      summarizeSamples([
        { rssMiB: 100, cpuPercent: 1 },
        { rssMiB: 120, cpuPercent: 2 },
        { rssMiB: 300, cpuPercent: 9 },
      ])
    ).toEqual({
      samples: 3,
      rssMiB: { mean: 173.33, p95: 300, max: 300 },
      cpuPercent: { mean: 4, p95: 9, max: 9 },
    });
  });
});
