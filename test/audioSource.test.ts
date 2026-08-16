import { describe, expect, test } from 'bun:test';
import {
  createBlobAudioSource,
  createRemoteAudioSource,
  discardFailedCache,
  getAudioSourceOriginsAfter,
  isCacheCorruptionError,
  normalizeAudioFormat,
  resolveAudioSource,
  toHowlSourceOptions,
} from '../src/utils/audioSource';

describe('音频格式识别', () => {
  test('缓存里的 FLAC 文件头会生成带正确 MIME 和 Howler 格式的 Blob', () => {
    const source = createBlobAudioSource(
      new Uint8Array([0x66, 0x4c, 0x61, 0x43, 0, 0]),
      () => 'blob:test-flac',
      'cache'
    );

    expect(source.format).toBe('flac');
    expect(source.mimeType).toBe('audio/flac');
    expect(toHowlSourceOptions(source)).toEqual({
      src: [source.url],
      format: 'flac',
    });
  });

  test('网易云返回的大小写和别名格式会被规范化', () => {
    expect(normalizeAudioFormat('FLAC')).toBe('flac');
    expect(normalizeAudioFormat('mpeg')).toBe('mp3');
    expect(normalizeAudioFormat('x-m4a')).toBe('m4a');
  });

  test('没有扩展名的第三方签名 URL 保留 MP3 兼容提示', () => {
    expect(
      createRemoteAudioSource('https://example.com/signed?id=1', {
        origin: 'unm',
        fallbackFormat: 'mp3',
      }).format
    ).toBe('mp3');
  });

  test('Ogg 容器会继续区分 Opus，RIFF 非 WAVE 不会误报音频', () => {
    expect(
      createBlobAudioSource(
        new TextEncoder().encode('OggS____OpusHead'),
        blob => blob
      ).format
    ).toBe('opus');
    expect(
      createBlobAudioSource(
        new TextEncoder().encode('RIFF____AVI '),
        blob => blob
      ).format
    ).toBeNull();
  });
});

describe('音源失败降级顺序', () => {
  test('坏缓存之后重新请求网易云，再尝试 UNM', () => {
    expect(getAudioSourceOriginsAfter('cache')).toEqual(['netease', 'unm']);
  });

  test('网易云直链格式不可用时仍会尝试 UNM，但不会无限循环', () => {
    expect(getAudioSourceOriginsAfter('netease')).toEqual(['unm']);
    expect(getAudioSourceOriginsAfter('unm')).toEqual([]);
  });

  test('坏缓存与空网易云直链会实际落到 UNM，且每层只请求一次', async () => {
    const calls: string[] = [];
    const source = await resolveAudioSource(
      {
        cache: async () => {
          calls.push('cache');
          throw new Error('失败后的降级不应重新读取坏缓存');
        },
        netease: async () => {
          calls.push('netease');
          throw new Error('直链请求失败');
        },
        unm: async () => {
          calls.push('unm');
          return createRemoteAudioSource('https://example.com/audio.mp3', {
            origin: 'unm',
          });
        },
      },
      'cache',
      origin => calls.push(`${origin}:error`)
    );

    expect(calls).toEqual(['netease', 'netease:error', 'unm']);
    expect(source?.origin).toBe('unm');
  });

  test('只有解码类错误才算缓存损坏，网络错误不能误删离线副本', () => {
    // MediaError: 3 = DECODE, 4 = SRC_NOT_SUPPORTED → 真损坏
    expect(isCacheCorruptionError(3)).toBe(true);
    expect(isCacheCorruptionError(4)).toBe(true);
    // 2 = NETWORK（Sidecar 重启 / 回环断开）以及未知形态 → 保留缓存
    expect(isCacheCorruptionError(2)).toBe(false);
    expect(isCacheCorruptionError(1)).toBe(false);
    expect(isCacheCorruptionError(undefined)).toBe(false);
    expect(isCacheCorruptionError('Decoding failed')).toBe(false);
  });

  test('删除毒缓存失败只记录错误，不阻断后续音源降级', async () => {
    const errors: string[] = [];
    expect(
      await discardFailedCache(
        async () => {
          throw new Error('IndexedDB busy');
        },
        123,
        error =>
          errors.push(error instanceof Error ? error.message : String(error))
      )
    ).toBe(false);
    expect(errors).toEqual(['IndexedDB busy']);
  });
});
