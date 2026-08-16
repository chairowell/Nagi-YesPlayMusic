import { getAppStore } from '@/stores/accessor';
import { isDesktopRuntime } from '@/utils/runtime';
import type { LyricsResponse } from '@/utils/lyrics';

export interface NeteasePlaybackSource {
  url: string;
  codec: string;
  actualBitrate: number;
  expectedBytes: number | null;
  expectedMd5: string | null;
}

// The API selects Hi-Res for quality values at or above 400000.
export function playbackBitrate(quality: number | 'flac'): number {
  return quality === 'flac' ? 350000 : quality;
}

export type PlaybackSettings = ReturnType<typeof getAppStore>['settings'];

// Same cascade the axios interceptor applies to every NCM call: env
// override, then the user's real-IP setting, then the web-only fallback.
function playbackRealIP(settings: PlaybackSettings): string | null {
  const envIP: unknown = import.meta.env['VUE_APP_REAL_IP'];
  if (typeof envIP === 'string' && envIP.length > 0) return envIP;
  if (settings.enableRealIP && settings.realIP) return settings.realIP;
  if (!isDesktopRuntime) return '211.161.244.70';
  return null;
}

function playbackProxy(settings: PlaybackSettings): string | null {
  const proxy = settings.proxyConfig;
  return proxy && ['HTTP', 'HTTPS'].includes(proxy.protocol)
    ? `${proxy.protocol}://${proxy.server}:${proxy.port}`
    : null;
}

export function unlockParams(settings: PlaybackSettings): URLSearchParams {
  const params = new URLSearchParams();
  const realIP = playbackRealIP(settings);
  if (realIP) params.set('realIP', realIP);
  const proxy = playbackProxy(settings);
  if (proxy) params.set('proxy', proxy);
  return params;
}

/**
 * Fetch a track's lyrics through the sidecar's typed endpoint (core::ncm's
 * lyric_new, shared with the TUI). Maps the flat payload back onto the
 * legacy LyricsResponse shape so lyricParser and the IndexedDB lyric cache
 * keep working unchanged. Throws on failure like the axios path it replaces.
 */
export async function fetchNeteaseLyrics(
  trackID: number
): Promise<LyricsResponse> {
  const params = unlockParams(getAppStore().settings);
  const queryString = params.size > 0 ? `?${params}` : '';
  const response = await fetch(
    `/api/native/playback/lyrics/${trackID}${queryString}`
  );
  if (!response.ok) {
    throw new Error(`歌词请求失败（HTTP ${response.status}）：${trackID}`);
  }
  const payload: unknown = await response.json();
  if (typeof payload !== 'object' || payload === null) {
    throw new Error(`歌词响应格式无效：${trackID}`);
  }
  const body = payload as Record<string, unknown>;
  const lrc = lyricSection(body['lrc']);
  const tlyric = lyricSection(body['tlyric']);
  const romalrc = lyricSection(body['romalrc']);
  return {
    ...(lrc ? { lrc } : {}),
    ...(tlyric ? { tlyric } : {}),
    ...(romalrc ? { romalrc } : {}),
  };
}

function lyricSection(value: unknown): { lyric: string } | null {
  return typeof value === 'string' && value.length > 0 ? { lyric: value } : null;
}

/**
 * Resolve a track's NetEase playback source through the sidecar's typed
 * endpoint. Classification (candidate matching, free-trial refusal,
 * rejected-vs-unavailable) happens server-side in core::ncm, shared with
 * the TUI. Returns null whenever the chain should move to the next origin.
 */
export async function resolveNeteasePlaybackSource(
  trackID: number
): Promise<NeteasePlaybackSource | null> {
  const settings = getAppStore().settings;
  const params = unlockParams(settings);
  params.set('bitrate', String(playbackBitrate(settings.musicQuality)));
  let payload: unknown;
  try {
    const response = await fetch(
      `/api/native/playback/source/${trackID}?${params}`
    );
    payload = await response.json();
  } catch (error) {
    console.warn(`[Player] 播放源解析请求失败：${trackID}`, error);
    return null;
  }
  if (typeof payload !== 'object' || payload === null) return null;
  const body = payload as Record<string, unknown>;
  switch (body['status']) {
    case 'ok':
      return typeof body['url'] === 'string' &&
        typeof body['codec'] === 'string' &&
        typeof body['actualBitrate'] === 'number'
        ? {
            url: body['url'],
            codec: body['codec'],
            actualBitrate: body['actualBitrate'],
            expectedBytes:
              typeof body['expectedBytes'] === 'number'
                ? body['expectedBytes']
                : null,
            expectedMd5:
              typeof body['expectedMd5'] === 'string'
                ? body['expectedMd5']
                : null,
          }
        : null;
    case 'unavailable':
      return null;
    case 'rejected':
      console.warn(
        `[Player] 网易云拒绝了播放源请求（code ${String(body['code'])}）：${trackID}`
      );
      return null;
    default:
      console.warn(`[Player] 播放源解析失败：${trackID}`, body);
      return null;
  }
}
