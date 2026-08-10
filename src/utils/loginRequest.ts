import type { AxiosRequestConfig } from 'axios';

export interface PasswordCredentials {
  password: string;
  md5_password?: string;
}

export interface PhoneCredentials extends PasswordCredentials {
  phone: string;
  countrycode?: string;
}

export interface EmailCredentials extends PasswordCredentials {
  email: string;
}

export function createPasswordLoginRequest(
  url: '/login/cellphone' | '/login',
  credentials: PhoneCredentials | EmailCredentials
): AxiosRequestConfig {
  return { url, method: 'post', data: credentials };
}
