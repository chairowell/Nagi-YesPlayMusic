import { isTauriRuntime } from '@/utils/runtime';
import { electronRenderer } from '@/services/desktopTransport';

export function normalizeExternalUrl(value) {
  let url;
  try {
    url = new URL(value);
  } catch {
    throw new Error('外链地址无效');
  }
  if (!['http:', 'https:'].includes(url.protocol)) {
    throw new Error('只允许打开 HTTP(S) 外链');
  }
  return url.href;
}

export async function openExternalUrl(
  value,
  {
    isTauri = isTauriRuntime,
    electronOpen = electronRenderer
      ? url => electronRenderer.invoke('openExternalUrl', url)
      : null,
    tauriOpen,
    browserOpen = url => window.open(url, '_blank', 'noopener,noreferrer'),
  } = {}
) {
  const url = normalizeExternalUrl(value);
  if (electronOpen) {
    await electronOpen(url);
    return;
  }
  if (isTauri) {
    const open = tauriOpen ?? (await import('@tauri-apps/plugin-opener')).openUrl;
    await open(url);
    return;
  }
  browserOpen(url);
}

export async function openExternalUrlSafely(value) {
  try {
    await openExternalUrl(value);
    return true;
  } catch (error) {
    console.error('[desktop] 无法打开外链：', error);
    return false;
  }
}

export function createExternalLinkClickHandler(opener = openExternalUrlSafely) {
  return event => {
    if (event.defaultPrevented || event.button > 0) return;
    const anchor = event.target?.closest?.('a[href]');
    if (!anchor) return;

    let url;
    try {
      url = normalizeExternalUrl(anchor.href);
    } catch {
      return;
    }
    if (globalThis.location?.origin === new URL(url).origin) return;

    event.preventDefault();
    Promise.resolve()
      .then(() => opener(url))
      .catch(error => {
        console.error('[desktop] 无法打开外链：', error);
      });
  };
}
