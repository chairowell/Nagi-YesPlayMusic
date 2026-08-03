import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { buildArtworkURL } from '../src/utils/artwork';

const trackListItemSource = readFileSync(
  new URL('../src/components/TrackListItem.vue', import.meta.url),
  'utf8'
);
const filtersSource = readFileSync(
  new URL('../src/utils/filters.js', import.meta.url),
  'utf8'
);

describe('歌单歌曲封面', () => {
  test('列表项复用统一图片处理，避免 WKWebView 拦截 HTTP 封面', () => {
    expect(
      buildArtworkURL('http://p1.music.126.net/cover.jpg', 224)
    ).toBe('https://p1.music.126.net/cover.jpg?param=224y224');
    expect(trackListItemSource).toContain('return buildArtworkURL(image, 224);');
    expect(trackListItemSource).not.toContain(
      "return image + '?param=224y224';"
    );
  });

  test('旧图片过滤器与新播放器共用同一个 URL 规则', () => {
    expect(filtersSource).toContain(
      "import { buildArtworkURL } from '@/utils/artwork';"
    );
    expect(filtersSource).toContain('return buildArtworkURL(imgUrl, size);');
  });
});
