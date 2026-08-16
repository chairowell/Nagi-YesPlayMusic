import { getAppStore } from '@/stores/accessor';
import { unlockParams } from '@/services/playbackSource';

/**
 * Typed library endpoints (core::ncm, shared with the TUI). Mutations throw
 * on refusal like the axios wrappers they replace, so caller catch blocks
 * keep working.
 */
async function libraryRequest(
  path: string,
  method: 'GET' | 'POST',
  fields: Record<string, string>
): Promise<unknown> {
  const params = unlockParams(getAppStore().settings);
  for (const [key, value] of Object.entries(fields)) {
    params.set(key, value);
  }
  const response = await fetch(`/api/native/library/${path}?${params}`, {
    method,
  });
  if (!response.ok) {
    throw new Error(`资料库请求失败（HTTP ${response.status}）：${path}`);
  }
  if (response.status === 204) return undefined;
  return response.json() as Promise<unknown>;
}

/** Ordered as NCM answers: most recently liked first. */
export async function fetchLikedSongIds(
  uid: number
): Promise<{ ids?: number[] }> {
  const payload = await libraryRequest('liked-ids', 'GET', {
    uid: String(uid),
  });
  if (typeof payload !== 'object' || payload === null) return {};
  const ids = (payload as Record<string, unknown>)['ids'];
  return Array.isArray(ids)
    ? { ids: ids.filter((id): id is number => typeof id === 'number') }
    : {};
}

export async function likeTrack(params: {
  id: number;
  like?: boolean;
}): Promise<void> {
  await libraryRequest('like', 'POST', {
    id: String(params.id),
    like: String(params.like ?? true),
  });
}

export async function trashFM(id: number): Promise<void> {
  await libraryRequest('fm-trash', 'POST', { id: String(id) });
}
