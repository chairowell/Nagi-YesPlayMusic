import request from '@/utils/request';
import { createPasswordLoginRequest } from '@/utils/loginRequest';
import type { EmailCredentials, PhoneCredentials } from '@/utils/loginRequest';
import type { ApiResponse } from './types';
import {
  decodeApiResponse,
  decodeNumber,
  decodeOptionalString,
  decodeRecord,
  decodeString,
} from './decoders';
import type { Decoder } from './decoders';

export interface LoginResponse extends ApiResponse {
  code: number;
  cookie?: string;
  msg?: string;
  message?: string;
}

const decodeLoginResponse: Decoder<LoginResponse> = (input, context) => {
  const response = decodeRecord(input, context);
  const cookie = decodeOptionalString(response['cookie'], context, '$.cookie');
  const msg = decodeOptionalString(response['msg'], context, '$.msg');
  const message = decodeOptionalString(
    response['message'],
    context,
    '$.message'
  );
  return {
    ...response,
    code: decodeNumber(response['code'], context, '$.code'),
    ...(cookie === undefined ? {} : { cookie }),
    ...(msg === undefined ? {} : { msg }),
    ...(message === undefined ? {} : { message }),
  };
};

const decodeQrCodeKeyResponse: Decoder<{
  code: number;
  data: { unikey: string };
}> = (input, context) => {
  const response = decodeRecord(input, context);
  const data = decodeRecord(response['data'], context, '$.data');
  return {
    code: decodeNumber(response['code'], context, '$.code'),
    data: {
      unikey: decodeString(data['unikey'], context, '$.data.unikey'),
    },
  };
};

export function loginWithPhone(
  params: PhoneCredentials
): Promise<LoginResponse> {
  return request<LoginResponse>(
    createPasswordLoginRequest('/login/cellphone', params),
    decodeLoginResponse
  );
}

export function loginWithEmail(
  params: EmailCredentials
): Promise<LoginResponse> {
  return request<LoginResponse>(
    createPasswordLoginRequest('/login', params),
    decodeLoginResponse
  );
}

export function loginQrCodeKey() {
  return request<{ code: number; data: { unikey: string } }>(
    {
      url: '/login/qr/key',
      method: 'get',
      params: {
        timestamp: new Date().getTime(),
      },
    },
    decodeQrCodeKeyResponse
  );
}

export function loginQrCodeCreate(params: {
  key: string;
  qrimg?: string;
}): Promise<ApiResponse> {
  return request<ApiResponse>(
    {
      url: '/login/qr/create',
      method: 'get',
      params: {
        ...params,
        timestamp: new Date().getTime(),
      },
    },
    decodeApiResponse
  );
}

export function loginQrCodeCheck(key: string): Promise<LoginResponse> {
  return request<LoginResponse>(
    {
      url: '/login/qr/check',
      method: 'get',
      params: {
        key,
        timestamp: new Date().getTime(),
      },
    },
    decodeLoginResponse
  );
}

export function refreshCookie() {
  return request<ApiResponse>(
    {
      url: '/login/refresh',
      method: 'post',
    },
    decodeApiResponse
  );
}

export function logout() {
  return request<ApiResponse>(
    {
      url: '/logout',
      method: 'post',
    },
    decodeApiResponse
  );
}

export async function clearDesktopSession(): Promise<true> {
  const response = await fetch('/api/native/logout-session', {
    method: 'POST',
    credentials: 'same-origin',
    headers: { Accept: 'application/json' },
  });
  if (!response.ok) {
    throw new Error(`本机会话清理失败（HTTP ${response.status}）`);
  }
  return true;
}
