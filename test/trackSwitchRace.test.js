import { describe, expect, test } from 'bun:test';
import {
  createTrackSwitchGuard,
  runLatestTrackSwitch,
} from '../src/utils/trackSwitch';

function deferred() {
  let resolve;
  const promise = new Promise(done => {
    resolve = done;
  });
  return { promise, resolve };
}

describe('快速切歌事务', () => {
  test('新切歌会立即清场，并阻止旧请求晚到后覆盖音频', async () => {
    const guard = createTrackSwitchGuard();
    const firstDetail = deferred();
    const firstSource = deferred();
    const secondDetail = deferred();
    const secondSource = deferred();
    const events = [];

    const switchTrack = (name, detail, source) =>
      runLatestTrackSwitch(guard, {
        onBegin: () => events.push(`reset:${name}`),
        loadTrack: () => detail.promise,
        commitTrack: track => events.push(`track:${track.name}`),
        loadSource: () => source.promise,
        commitSource: audio => events.push(`audio:${audio}`),
      });

    const first = switchTrack('first', firstDetail, firstSource);
    expect(events).toEqual(['reset:first']);

    firstDetail.resolve({ name: 'first' });
    await Promise.resolve();
    expect(events).toEqual(['reset:first', 'track:first']);

    const second = switchTrack('second', secondDetail, secondSource);
    expect(events.at(-1)).toBe('reset:second');

    secondDetail.resolve({ name: 'second' });
    await Promise.resolve();
    secondSource.resolve('second');
    await second;

    firstSource.resolve('first');
    expect(await first).toBe(false);
    expect(events).toEqual([
      'reset:first',
      'track:first',
      'reset:second',
      'track:second',
      'audio:second',
    ]);
  });

  test('详情还没返回时连续切歌，旧详情也不能覆盖新歌曲', async () => {
    const guard = createTrackSwitchGuard();
    const firstDetail = deferred();
    const secondDetail = deferred();
    const committed = [];

    const first = runLatestTrackSwitch(guard, {
      loadTrack: () => firstDetail.promise,
      commitTrack: track => committed.push(track),
      loadSource: () => Promise.resolve('first-source'),
      commitSource: () => committed.push('first-audio'),
    });
    const second = runLatestTrackSwitch(guard, {
      loadTrack: () => secondDetail.promise,
      commitTrack: track => committed.push(track),
      loadSource: () => Promise.resolve('second-source'),
      commitSource: () => committed.push('second-audio'),
    });

    secondDetail.resolve('second-track');
    await second;
    firstDetail.resolve('first-track');

    expect(await first).toBe(false);
    expect(committed).toEqual(['second-track', 'second-audio']);
  });
});
