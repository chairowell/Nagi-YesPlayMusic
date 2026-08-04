import { describe, expect, test } from 'bun:test';
import {
  parseFlacStreamInfo,
  buildFloat32WavBlob,
} from '../src/utils/pcmSeekSource';

function makeFlacHeader({ sampleRate, channels }) {
  // 'fLaC' + STREAMINFO 块头(4B) + STREAMINFO 34B
  const bytes = new Uint8Array(4 + 4 + 34);
  bytes.set([0x66, 0x4c, 0x61, 0x43], 0);
  bytes[4] = 0x80; // last-metadata-block + type 0 (STREAMINFO)
  bytes[7] = 34;
  // 采样率 20 bits 从 STREAMINFO 第 10 字节起；声道数-1 占其后 3 bits
  const offset = 8 + 10;
  bytes[offset] = (sampleRate >> 12) & 0xff;
  bytes[offset + 1] = (sampleRate >> 4) & 0xff;
  bytes[offset + 2] = ((sampleRate & 0x0f) << 4) | (((channels - 1) & 0x07) << 1);
  return bytes.buffer;
}

describe('FLAC STREAMINFO 解析', () => {
  test('读出网易云无损常见的 44.1kHz 双声道', () => {
    expect(parseFlacStreamInfo(makeFlacHeader({ sampleRate: 44100, channels: 2 })))
      .toEqual({ sampleRate: 44100, channels: 2 });
  });

  test('读出 48kHz 单声道', () => {
    expect(parseFlacStreamInfo(makeFlacHeader({ sampleRate: 48000, channels: 1 })))
      .toEqual({ sampleRate: 48000, channels: 1 });
  });

  test('非 FLAC 或损坏数据返回 null 走降级路径', () => {
    expect(parseFlacStreamInfo(new Uint8Array([0x49, 0x44, 0x33, 0]).buffer)).toBeNull();
    expect(parseFlacStreamInfo(new ArrayBuffer(2))).toBeNull();
    expect(
      parseFlacStreamInfo(makeFlacHeader({ sampleRate: 0, channels: 1 }))
    ).toBeNull();
  });
});

describe('float32 WAV 打包', () => {
  const stubBuffer = {
    numberOfChannels: 2,
    length: 4,
    sampleRate: 44100,
    copyFromChannel(target, channel) {
      // 左声道 0.1,0.2,0.3,0.4；右声道 -0.1,-0.2,-0.3,-0.4
      const sign = channel === 0 ? 1 : -1;
      for (let i = 0; i < 4; i++) target[i] = sign * (i + 1) / 10;
    },
  };

  test('WAV 头字段与数据长度正确（IEEE float、双声道交织）', async () => {
    const blob = buildFloat32WavBlob(stubBuffer);
    const bytes = new DataView(await blob.arrayBuffer());

    expect(String.fromCharCode(...new Uint8Array(bytes.buffer, 0, 4))).toBe('RIFF');
    expect(String.fromCharCode(...new Uint8Array(bytes.buffer, 8, 4))).toBe('WAVE');
    expect(bytes.getUint16(20, true)).toBe(3); // IEEE float
    expect(bytes.getUint16(22, true)).toBe(2); // channels
    expect(bytes.getUint32(24, true)).toBe(44100);
    expect(bytes.getUint16(34, true)).toBe(32); // bits per sample
    expect(bytes.getUint32(46, true)).toBe(4); // fact: frames
    expect(bytes.getUint32(54, true)).toBe(4 * 2 * 4); // data bytes

    const pcm = new Float32Array(bytes.buffer.slice(58));
    expect(Array.from(pcm.slice(0, 4)).map(v => Math.round(v * 10) / 10)).toEqual([
      0.1, -0.1, 0.2, -0.2,
    ]);
    expect(blob.type).toBe('audio/wav');
  });

  test('RIFF 总长度自洽', async () => {
    const blob = buildFloat32WavBlob(stubBuffer);
    const bytes = new DataView(await blob.arrayBuffer());
    expect(bytes.getUint32(4, true)).toBe(blob.size - 8);
  });
});
