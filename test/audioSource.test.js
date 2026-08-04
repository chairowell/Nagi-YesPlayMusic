import { describe, expect, test } from 'bun:test';
import {
  createBlobAudioSource,
  createRemoteAudioSource,
  discardFailedCache,
  getAudioSourceOriginsAfter,
  normalizeAudioFormat,
  resolveAudioSource,
  toHowlSourceOptions,
} from '../src/utils/audioSource';

describe('音频格式识别', () => {
  test('缓存里的 FLAC 文件头会生成带正确 MIME 和 Howler 格式的 Blob', () => {
    const source = createBlobAudioSource(
      new Uint8Array([0x66, 0x4c, 0x61, 0x43, 0, 0]),
      url => url,
      'cache'
    );

    expect(source.format).toBe('flac');
    expect(source.mimeType).toBe('audio/flac');
    expect(toHowlSourceOptions(source)).toEqual({
      src: [source.url],
      format: 'flac',
      html5: false,
    });
  });

  test('缓存命中走 Web Audio，远程与 UNM 源保持 HTML5 流式', () => {
    // WebKit/AVPlayer 对流式 FLAC 的 seek 会把落点放在请求位置前几秒、
    // currentTime 却按请求值计时；Web Audio 全量解码后 seek 是纯数学，
    // 才能保证歌词时钟读到的位置就是真实出声位置。
    const cached = createBlobAudioSource(
      new Uint8Array([0x66, 0x4c, 0x61, 0x43, 0, 0]),
      url => url,
      'cache'
    );
    expect(toHowlSourceOptions(cached).html5).toBe(false);

    // 远程 CDN 无 CORS 头，Web Audio 的 XHR 拉不动，只能 HTML5 流式起播
    const remote = createRemoteAudioSource('https://m804.music.126.net/a.flac', {
      origin: 'netease',
      format: 'flac',
    });
    expect(toHowlSourceOptions(remote).html5).toBe(true);

    // UNM 拿到的容器五花八门（B 站 m4s 等），decodeAudioData 可能不认，
    // 保持 HTML5 交给系统媒体栈
    const unm = createBlobAudioSource(
      new TextEncoder().encode('OggS____OpusHead'),
      url => url,
      'unm'
    );
    expect(toHowlSourceOptions(unm).html5).toBe(true);
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
    const calls = [];
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
          return { origin: 'unm', url: 'https://example.com/audio.mp3' };
        },
      },
      'cache',
      origin => calls.push(`${origin}:error`)
    );

    expect(calls).toEqual(['netease', 'netease:error', 'unm']);
    expect(source.origin).toBe('unm');
  });

  test('删除毒缓存失败只记录错误，不阻断后续音源降级', async () => {
    const errors = [];
    expect(
      await discardFailedCache(
        async () => {
          throw new Error('IndexedDB busy');
        },
        123,
        error => errors.push(error.message)
      )
    ).toBe(false);
    expect(errors).toEqual(['IndexedDB busy']);
  });
});
