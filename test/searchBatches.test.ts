import { expect, test } from 'bun:test';
import { settleIndependentRequests } from '../src/services/searchBatches';

test('搜索批次保留成功结果并单独报告失败请求', async () => {
  const failure = new Error('MV endpoint unavailable');
  const result = await settleIndependentRequests([
    Promise.resolve({ type: 'artists', count: 3 }),
    Promise.reject(failure),
    Promise.resolve({ type: 'tracks', count: 16 }),
  ]);

  expect(result.values).toEqual([
    { type: 'artists', count: 3 },
    { type: 'tracks', count: 16 },
  ]);
  expect(result.errors).toEqual([failure]);
});
