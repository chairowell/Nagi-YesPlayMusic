import { isTauriRuntime } from '@/utils/runtime';

type ExternalOpener = (url: string) => unknown | Promise<unknown>;

interface OpenExternalOptions {
  isTauri?: boolean;
  tauriOpen?: ExternalOpener;
  browserOpen?: (url: string) => unknown;
}

interface ClosestTarget {
  closest(selector: string): unknown;
}

interface ExternalLinkActivationEvent {
  altKey?: boolean;
  button: number;
  ctrlKey?: boolean;
  defaultPrevented: boolean;
  metaKey?: boolean;
  shiftKey?: boolean;
  target: unknown;
  type: string;
  preventDefault(): void;
}

function hasClosest(target: unknown): target is ClosestTarget {
  return (
    typeof target === 'object' &&
    target !== null &&
    'closest' in target &&
    typeof target.closest === 'function'
  );
}

export function normalizeExternalUrl(value: unknown): string {
  let url: URL;
  try {
    url = new URL(String(value));
  } catch {
    throw new Error('外链地址无效');
  }
  if (!['http:', 'https:'].includes(url.protocol)) {
    throw new Error('只允许打开 HTTP(S) 外链');
  }
  return url.href;
}

export async function openExternalUrl(
  value: unknown,
  {
    isTauri = isTauriRuntime,
    tauriOpen,
    browserOpen = url => window.open(url, '_blank', 'noopener,noreferrer'),
  }: OpenExternalOptions = {}
): Promise<void> {
  const url = normalizeExternalUrl(value);
  if (isTauri) {
    const open =
      tauriOpen ?? (await import('@tauri-apps/plugin-opener')).openUrl;
    await open(url);
    return;
  }
  browserOpen(url);
}

export async function openExternalUrlSafely(value: unknown): Promise<boolean> {
  try {
    await openExternalUrl(value);
    return true;
  } catch (error) {
    console.error('[desktop] 无法打开外链：', error);
    return false;
  }
}

export function createExternalLinkClickHandler(
  opener: ExternalOpener = openExternalUrlSafely
): (event: ExternalLinkActivationEvent) => void {
  return (event: ExternalLinkActivationEvent) => {
    const supportedButton =
      event.type === 'auxclick' ? event.button === 1 : event.button === 0;
    if (event.defaultPrevented || !supportedButton) return;
    const anchor = hasClosest(event.target)
      ? event.target.closest('a[href]')
      : null;
    if (
      typeof anchor !== 'object' ||
      anchor === null ||
      !('href' in anchor) ||
      typeof anchor.href !== 'string'
    ) {
      return;
    }

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
