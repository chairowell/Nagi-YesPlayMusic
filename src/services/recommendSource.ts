import { getAppStore } from '@/stores/accessor';
import { unlockParams } from '@/services/playbackSource';
import { adaptTrackItems } from '@/services/songItems';
import { mapTrackPlayableStatus } from '@/utils/common';
import type { Track } from '@/types/domain';

/**
 * Daily recommendations through the sidecar (core::ncm, shared with the
 * TUI). Privileges arrive embedded per song; playable status is computed
 * client-side as everywhere else.
 */
export async function fetchDailyRecommendTracks(): Promise<Track[]> {
  const params = unlockParams(getAppStore().settings);
  const query = params.size > 0 ? `?${params}` : '';
  const response = await fetch(`/api/native/recommend/daily-songs${query}`);
  if (!response.ok) {
    throw new Error(`每日推荐请求失败（HTTP ${response.status}）`);
  }
  const payload: unknown = await response.json();
  const items =
    typeof payload === 'object' &&
    payload !== null &&
    Array.isArray((payload as Record<string, unknown>)['data'])
      ? ((payload as Record<string, unknown>)['data'] as unknown[]).filter(
          (item): item is Record<string, unknown> =>
            typeof item === 'object' && item !== null
        )
      : [];
  return mapTrackPlayableStatus(adaptTrackItems(items));
}
