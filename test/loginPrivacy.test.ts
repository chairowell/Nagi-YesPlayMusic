import { describe, expect, test } from 'bun:test';
import { createPasswordLoginRequest } from '../src/utils/loginRequest';

describe('账号密码请求隐私', () => {
  test('手机号凭据只进入 POST body，不进入 URL 查询参数', () => {
    const credentials = {
      phone: '13800000000',
      password: 'fakePassword',
      md5_password: 'secret-hash',
    };

    expect(createPasswordLoginRequest('/login/cellphone', credentials)).toEqual(
      {
        url: '/login/cellphone',
        method: 'post',
        data: credentials,
      }
    );
  });

  test('邮箱凭据只进入 POST body，不进入 URL 查询参数', () => {
    const credentials = {
      email: 'user@example.com',
      password: 'fakePassword',
      md5_password: 'secret-hash',
    };

    expect(createPasswordLoginRequest('/login', credentials)).toEqual({
      url: '/login',
      method: 'post',
      data: credentials,
    });
  });
});
