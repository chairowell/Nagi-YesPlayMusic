import { describe, expect, test } from 'bun:test';
import {
  parseFlacStreamInfo,
  buildFloat32WavBlob,
  discardPreciseWav,
  requestPreciseWavURL,
} from '../src/utils/pcmSeekSource';

interface FlacHeaderOptions {
  sampleRate: number;
  channels: number;
  bitsPerSample?: number;
}

interface FetchCall {
  u: string;
  method: string | undefined;
}

function makeFlacHeader({
  sampleRate,
  channels,
  bitsPerSample = 16,
}: FlacHeaderOptions): ArrayBuffer {
  // 'fLaC' + a 4-byte block header + 34-byte STREAMINFO.
  const bytes = new Uint8Array(4 + 4 + 34);
  bytes.set([0x66, 0x4c, 0x61, 0x43], 0);
  bytes[4] = 0x80; // last-metadata-block + type 0 (STREAMINFO)
  bytes[7] = 34;
  // Pack sample rate, channel count, and bit depth per STREAMINFO.
  const offset = 8 + 10;
  const bits = bitsPerSample - 1;
  bytes[offset] = (sampleRate >> 12) & 0xff;
  bytes[offset + 1] = (sampleRate >> 4) & 0xff;
  bytes[offset + 2] =
    ((sampleRate & 0x0f) << 4) |
    (((channels - 1) & 0x07) << 1) |
    ((bits >> 4) & 0x01);
  bytes[offset + 3] = (bits & 0x0f) << 4;
  return bytes.buffer;
}

describe('FLAC STREAMINFO 解析', () => {
  test('读出网易云无损常见的 44.1kHz 双声道 24-bit', () => {
    expect(
      parseFlacStreamInfo(
        makeFlacHeader({ sampleRate: 44100, channels: 2, bitsPerSample: 24 })
      )
    ).toEqual({ sampleRate: 44100, channels: 2, bitsPerSample: 24 });
  });

  test('读出 48kHz 单声道 16-bit', () => {
    expect(
      parseFlacStreamInfo(makeFlacHeader({ sampleRate: 48000, channels: 1 }))
    ).toEqual({ sampleRate: 48000, channels: 1, bitsPerSample: 16 });
  });

  test('非 FLAC 或损坏数据返回 null 走降级路径', () => {
    expect(
      parseFlacStreamInfo(new Uint8Array([0x49, 0x44, 0x33, 0]).buffer)
    ).toBeNull();
    expect(parseFlacStreamInfo(new ArrayBuffer(2))).toBeNull();
    expect(
      parseFlacStreamInfo(makeFlacHeader({ sampleRate: 0, channels: 1 }))
    ).toBeNull();
  });
});

describe('sidecar 精确 WAV 请求', () => {
  test('成功时返回 Range URL，并带上正确的位深参数', async () => {
    const calls: FetchCall[] = [];
    const url = await requestPreciseWavURL(
      42,
      new ArrayBuffer(8),
      24,
      async (u, init) => {
        calls.push({ u, method: init.method });
        return { ok: true, json: async () => ({ url: '/precise-wav/42.wav' }) };
      }
    );
    expect(url).toBe('/precise-wav/42.wav');
    expect(calls[0]).toEqual({ u: '/precise-wav/42?bits=24', method: 'POST' });
  });

  test('sidecar 不可达或响应异常一律返回 null，不外抛', async () => {
    expect(
      await requestPreciseWavURL(42, new ArrayBuffer(8), 16, async () => ({
        ok: false,
        json: async () => ({}),
      }))
    ).toBeNull();
    expect(
      await requestPreciseWavURL(42, new ArrayBuffer(8), 16, async () => {
        throw new Error('ECONNREFUSED');
      })
    ).toBeNull();
    expect(
      await requestPreciseWavURL(42, new ArrayBuffer(8), 16, null)
    ).toBeNull();
  });

  test('切歌清扫发 DELETE，失败静默不影响切歌流程', async () => {
    const calls: FetchCall[] = [];
    expect(
      await discardPreciseWav(async (u, init) => {
        calls.push({ u, method: init.method });
        return { ok: true };
      })
    ).toBe(true);
    expect(calls).toEqual([{ u: '/precise-wav', method: 'DELETE' }]);
    expect(
      await discardPreciseWav(async () => {
        throw new Error('sidecar 不在');
      })
    ).toBe(false);
    expect(await discardPreciseWav(null)).toBe(false);
  });
});

describe('float32 WAV 打包', () => {
  const stubBuffer: Parameters<typeof buildFloat32WavBlob>[0] = {
    numberOfChannels: 2,
    length: 4,
    sampleRate: 44100,
    copyFromChannel(target, channel) {
      // Left channel is positive; right channel mirrors it.
      const sign = channel === 0 ? 1 : -1;
      for (let i = 0; i < 4; i++) target[i] = (sign * (i + 1)) / 10;
    },
  };

  test('WAV 头字段与数据长度正确（IEEE float、双声道交织）', async () => {
    const blob = buildFloat32WavBlob(stubBuffer);
    const bytes = new DataView(await blob.arrayBuffer());

    expect(String.fromCharCode(...new Uint8Array(bytes.buffer, 0, 4))).toBe(
      'RIFF'
    );
    expect(String.fromCharCode(...new Uint8Array(bytes.buffer, 8, 4))).toBe(
      'WAVE'
    );
    expect(bytes.getUint16(20, true)).toBe(3); // IEEE float
    expect(bytes.getUint16(22, true)).toBe(2); // channels
    expect(bytes.getUint32(24, true)).toBe(44100);
    expect(bytes.getUint16(34, true)).toBe(32); // bits per sample
    expect(bytes.getUint32(46, true)).toBe(4); // fact: frames
    expect(bytes.getUint32(54, true)).toBe(4 * 2 * 4); // data bytes

    const pcm = new Float32Array(bytes.buffer.slice(58));
    expect(
      Array.from(pcm.slice(0, 4)).map(v => Math.round(v * 10) / 10)
    ).toEqual([0.1, -0.1, 0.2, -0.2]);
    expect(blob.type).toBe('audio/wav');
  });

  test('RIFF 总长度自洽', async () => {
    const blob = buildFloat32WavBlob(stubBuffer);
    const bytes = new DataView(await blob.arrayBuffer());
    expect(bytes.getUint32(4, true)).toBe(blob.size - 8);
  });
});
