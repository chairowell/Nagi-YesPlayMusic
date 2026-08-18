import { getAppStore } from '@/stores/accessor';
import { nativeFetch } from '@/utils/nativeFetch';
import { unlockParams } from '@/services/playbackSource';
import type { Artist, Track } from '@/types/domain';

/**
 * Personal FM through the sidecar (core::ncm, shared with the TUI). The
 * adapter reproduces the legacy `/personal_fm` item shape (artists/album/
 * duration naming) so FMCard and the player-info consumers stay unchanged.
 */
export async function fetchPersonalFM(): Promise<{ data: Track[] }> {
  const params = unlockParams(getAppStore().settings);
  const query = params.size > 0 ? `?${params}` : '';
  const response = await nativeFetch(`/api/native/fm/personal${query}`);
  if (!response.ok) {
    throw new Error(`私人FM请求失败（HTTP ${response.status}）`);
  }
  const payload: unknown = await response.json();
  const items =
    typeof payload === 'object' &&
    payload !== null &&
    Array.isArray((payload as Record<string, unknown>)['data'])
      ? ((payload as Record<string, unknown>)['data'] as unknown[]).filter(
          (item): item is Record<string, unknown> =>
            typeof item === 'object' &&
            item !== null &&
            typeof (item as Record<string, unknown>)['id'] === 'number'
        )
      : [];
  return {
    data: items.map(item => {
      const album =
        typeof item['album'] === 'object' && item['album'] !== null
          ? (item['album'] as Record<string, unknown>)
          : {};
      const track: Track = {
        id: item['id'] as number,
        ...(typeof item['name'] === 'string' ? { name: item['name'] } : {}),
        artists: Array.isArray(item['artists'])
          ? (item['artists'] as Artist[])
          : [],
        album: {
          id: typeof album['id'] === 'number' ? album['id'] : 0,
          ...(typeof album['name'] === 'string' ? { name: album['name'] } : {}),
          ...(typeof album['picUrl'] === 'string'
            ? { picUrl: album['picUrl'] }
            : {}),
        },
        duration: typeof item['durationMs'] === 'number' ? item['durationMs'] : 0,
      };
      return track;
    }),
  };
}
