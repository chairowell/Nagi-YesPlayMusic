import { isTauriRuntime } from '@/utils/runtime';
import { nativeFetch } from '@/utils/nativeFetch';
import type { Track } from '@/types/domain';

export interface UnblockedSong {
  url: string;
  source: string;
  [key: string]: unknown;
}

export async function requestUnblockedSong(
  sourceListString: string | undefined,
  track: Track,
  context: Record<string, unknown>
): Promise<UnblockedSong | null> {
  if (!isTauriRuntime) return null;

  try {
    const response = await nativeFetch('/api/native/unblock-music', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ sourceListString, track, context }),
    });
    if (!response.ok) return null;
    const payload: unknown = await response.json();
    return typeof payload === 'object' &&
      payload !== null &&
      'url' in payload &&
      typeof payload.url === 'string' &&
      'source' in payload &&
      typeof payload.source === 'string'
      ? { ...payload, url: payload.url, source: payload.source }
      : null;
  } catch (error) {
    console.warn('[UNM] Tauri sidecar request failed', error);
    return null;
  }
}
