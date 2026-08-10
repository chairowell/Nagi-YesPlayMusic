import { describe, expect, test } from 'bun:test';
import { fetchLikedSongIdsForUser } from '../src/stores/fetchLikedSongs';
import { resolveScrollingState } from '../src/stores/stateTransitions';

describe('全局状态行为', () => {
  test('获取喜欢歌曲时向接口传递标量用户 ID，并提交返回结果', async () => {
    const requestedIds: number[] = [];
    const ids = await fetchLikedSongIdsForUser(123, {
      isLooseLoggedIn: () => true,
      isAccountLoggedIn: () => true,
      fetchLikedSongIds: async userId => {
        requestedIds.push(userId);
        return { ids: [11, 22] };
      },
    });

    expect(requestedIds).toEqual([123]);
    expect(ids).toEqual([11, 22]);
  });

  test('显式关闭滚动时保持关闭，不会因当前状态为 false 而反向打开', () => {
    expect(resolveScrollingState(false, false)).toBe(false);
  });
});
