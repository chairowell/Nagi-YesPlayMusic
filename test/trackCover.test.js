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
const lyricsSource = readFileSync(
  new URL('../src/views/lyrics.vue', import.meta.url),
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

  test('迷你播放条的封面按自己的尺寸取图，不跟大歌词页共用 1024', () => {
    // 58px 的小封面下 1024×1024，比菜单栏那张 64px 大两个数量级；
    // 切歌时菜单栏秒换、播放条慢半拍就是在等这张大图。
    expect(lyricsSource).toContain(
      'return buildArtworkURL(this.player.currentTrack?.al?.picUrl, 128);'
    );
    expect(lyricsSource).toContain(
      '<img class="mini-cover" :src="miniImageUrl" />'
    );
    // 这张永远在视口里，懒加载只会推迟开始下载
    expect(lyricsSource).not.toContain('class="mini-cover" :src="miniImageUrl" loading');
  });
});
