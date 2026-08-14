import { describe, expect, test } from 'bun:test';
import {
  createTrackSwitchGuard,
  runLatestTrackSwitch,
} from '../src/utils/trackSwitch';
import { reportNeteaseScrobble } from '../src/utils/scrobbleReport';

function deferred<T>() {
  let resolve: (value: T | PromiseLike<T>) => void = () => {};
  const promise = new Promise<T>(done => {
    resolve = done;
  });
  return { promise, resolve };
}

describe('快速切歌事务', () => {
  test('网易云上报失败会被接住，不阻塞切歌', async () => {
    let rejectReport: (error: Error) => void = () => {};
    const pendingReport = new Promise<never>((_, reject) => {
      rejectReport = reject;
    });
    const failures: unknown[] = [];
    const switched: string[] = [];

    const result = reportNeteaseScrobble(
      { id: 42, sourceid: 7, time: 31 },
      () => pendingReport,
      error => failures.push(error)
    );
    const switchResult = await runLatestTrackSwitch(createTrackSwitchGuard(), {
      loadTrack: async () => 'next track',
      commitTrack: track => switched.push(track),
      loadSource: async () => 'next source',
      commitSource: source => switched.push(source),
    });

    expect(result).toBeUndefined();
    expect(switchResult).toBe(true);
    expect(switched).toEqual(['next track', 'next source']);
    const failure = new Error('offline');
    rejectReport(failure);
    await Promise.resolve();
    expect(failures).toEqual([failure]);
  });

  test('新切歌会立即清场，并阻止旧请求晚到后覆盖音频', async () => {
    const guard = createTrackSwitchGuard();
    const firstDetail = deferred<{ name: string }>();
    const firstSource = deferred<string>();
    const secondDetail = deferred<{ name: string }>();
    const secondSource = deferred<string>();
    const events: string[] = [];

    const switchTrack = (
      name: string,
      detail: ReturnType<typeof deferred<{ name: string }>>,
      source: ReturnType<typeof deferred<string>>
    ) =>
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
    const firstDetail = deferred<string>();
    const secondDetail = deferred<string>();
    const committed: string[] = [];

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
