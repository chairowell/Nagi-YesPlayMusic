import { describe, expect, test } from 'bun:test';
import { commitHowlerSeek } from '../src/utils/playbackSeek';

describe('播放 seek 落点同步', () => {
  test('提交后读回媒体实际落点，不把请求时间冒充成播放时间', () => {
    let actualPosition = 12;
    const howler = {
      seek(value) {
        if (value !== undefined) {
          // 模拟 WebKit / 流媒体把请求时间修正到实际可解码位置。
          actualPosition = 41.75;
          return this;
        }
        return actualPosition;
      },
    };

    expect(commitHowlerSeek(howler, 43)).toBe(41.75);
  });

  test('媒体尚未返回有效落点时保留有限的请求时间', () => {
    const howler = {
      seek(value) {
        return value === undefined ? Number.NaN : this;
      },
    };

    expect(commitHowlerSeek(howler, 43)).toBe(43);
    expect(commitHowlerSeek(null, 43)).toBeNull();
  });

  test('越过歌曲开头的 seek 会落到零点，而不是忽略操作', () => {
    let receivedPosition = null;
    const howler = {
      seek(value) {
        if (value !== undefined) {
          receivedPosition = value;
          return this;
        }
        return receivedPosition;
      },
    };

    expect(commitHowlerSeek(howler, -5)).toBe(0);
    expect(receivedPosition).toBe(0);
  });
});
