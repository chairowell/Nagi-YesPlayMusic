import { describe, expect, test } from 'bun:test';
import {
  ApiContractError,
  decodeCodeResponse,
  decodeDailyTracksResponse,
  decodeTrackCollectionResponse,
  decodeUserProfile,
} from '../src/api/decoders';

describe('API response decoders', () => {
  test('accepts daily recommendations after the upstream privileges field was removed', () => {
    const result = decodeDailyTracksResponse(
      {
        code: 200,
        data: {
          dailySongs: [{ id: 42, name: 'Track' }],
          recommendReasons: [],
        },
      },
      { url: '/recommend/songs' }
    );

    expect(result.data.dailySongs[0]?.id).toBe(42);
    expect(result.data.privileges).toBeUndefined();
  });

  test('still validates daily recommendation privileges when upstream returns them', () => {
    expect(() =>
      decodeDailyTracksResponse(
        {
          code: 200,
          data: {
            dailySongs: [{ id: 42 }],
            privileges: [{ id: '42' }],
          },
        },
        { url: '/recommend/songs' }
      )
    ).toThrow('/recommend/songs 的 $.data.privileges[0].id');
  });

  test('preserves valid response data after checking nested track identities', () => {
    const result = decodeTrackCollectionResponse(
      {
        songs: [{ id: 42, name: 'Track' }],
        privileges: [{ id: 42, pl: 320000 }],
        extra: 'kept',
      },
      { url: '/song/detail' }
    );

    expect(result.songs[0]?.id).toBe(42);
    expect(result.privileges?.[0]?.id).toBe(42);
    expect(result['extra']).toBe('kept');
  });

  test('reports the endpoint and exact nested field for malformed data', () => {
    expect(() =>
      decodeTrackCollectionResponse(
        { songs: [{ id: '42' }] },
        { url: '/song/detail' }
      )
    ).toThrow('API 响应契约错误：/song/detail 的 $.songs[0].id 应为有限数字');
  });

  test('rejects non-object and malformed scalar responses at the boundary', () => {
    expect(() => decodeCodeResponse(null, { url: '/playlist/delete' })).toThrow(
      ApiContractError
    );
    expect(() =>
      decodeCodeResponse({ code: '200' }, { url: '/playlist/delete' })
    ).toThrow('/playlist/delete 的 $.code');
    expect(() =>
      decodeUserProfile(
        { userId: 7, nickname: 7 },
        { url: '/user/account' },
        '$.profile'
      )
    ).toThrow('/user/account 的 $.profile.nickname');
  });
});
