import { expect, mock, test } from 'bun:test';
import { createNativeFetch } from '../src/utils/nativeFetch';
import { handleNcmSessionExpiry } from '../src/utils/sessionExpiry';

test('挂起的请求在超时后拒绝，而不是永远悬死', async () => {
  const hang = createNativeFetch({
    timeoutMs: 20,
    fetcher: (_input, init) =>
      new Promise<Response>((_resolve, reject) => {
        init?.signal?.addEventListener('abort', () =>
          reject(new DOMException('aborted', 'AbortError'))
        );
      }),
    onUnauthorized: () => undefined,
  });
  await expect(hang('/api/native/search?keywords=x')).rejects.toThrow(
    '请求超时'
  );
});

test('401 + 网易云过期体触发登出跳转，其他 401 不动登录态', async () => {
  const logout = mock(() => undefined);
  const navigate = mock((_route: 'login' | 'loginAccount') => undefined);
  const wrapped = createNativeFetch({
    fetcher: (input, _init) =>
      Promise.resolve(
        new Response(
          JSON.stringify(
            String(input).includes('expired')
              ? { code: 301, msg: '需要登录' }
              : { message: 'native API authentication failed' }
          ),
          { status: 401 }
        )
      ),
    onUnauthorized: data => {
      handleNcmSessionExpiry(data, {
        loginRoute: 'loginAccount',
        logout,
        navigate,
      });
    },
  });

  // Sidecar native-token boundary failure: same status, different body.
  const boundary = await wrapped('/api/native/library/liked-ids');
  expect(boundary.status).toBe(401);
  expect(logout).toHaveBeenCalledTimes(0);

  const expired = await wrapped('/api/native/library/liked-ids?expired');
  expect(expired.status).toBe(401);
  expect(logout).toHaveBeenCalledTimes(1);
  expect(navigate).toHaveBeenCalledWith('loginAccount');
  // The response body stays readable for the caller.
  const body = (await expired.json()) as { code: number };
  expect(body.code).toBe(301);
});

test('正常响应原样透传，包括非 401 的失败状态', async () => {
  const onUnauthorized = mock((_data: unknown) => undefined);
  const wrapped = createNativeFetch({
    fetcher: () =>
      Promise.resolve(
        new Response(JSON.stringify({ ids: [1] }), { status: 200 })
      ),
    onUnauthorized,
  });
  const response = await wrapped('/api/native/library/liked-ids');
  expect(response.status).toBe(200);
  expect(onUnauthorized).toHaveBeenCalledTimes(0);

  const failing = createNativeFetch({
    fetcher: () => Promise.resolve(new Response('bad', { status: 502 })),
    onUnauthorized,
  });
  const failure = await failing('/api/native/search');
  expect(failure.status).toBe(502);
  expect(onUnauthorized).toHaveBeenCalledTimes(0);
});
