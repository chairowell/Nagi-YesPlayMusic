import { electronRenderer } from '@/services/desktopTransport';
import { isTauriRuntime } from '@/utils/runtime';

export async function requestUnblockedSong(sourceListString, track, context) {
  if (electronRenderer) {
    return electronRenderer.invoke(
      'unblock-music',
      sourceListString,
      track,
      context
    );
  }
  if (!isTauriRuntime) return null;

  try {
    const response = await fetch('/api/native/unblock-music', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ sourceListString, track, context }),
    });
    if (!response.ok) return null;
    return response.json();
  } catch (error) {
    console.warn('[UNM] Tauri sidecar 请求失败', error);
    return null;
  }
}
