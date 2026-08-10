// Last.fm API documents 👉 https://www.last.fm/api

import axios from 'axios';
import md5 from 'crypto-js/md5';
import { isDesktopRuntime } from '@/utils/runtime';
import type { AxiosResponse } from 'axios';
import type { UnknownRecord } from '@/types/domain';
import { decodeRecord, decodeString } from './decoders';

const apiKey = import.meta.env['VUE_APP_LASTFM_API_KEY'] ?? '';
const apiSharedSecret =
  import.meta.env['VUE_APP_LASTFM_API_SHARED_SECRET'] ?? '';
const url = 'https://ws.audioscrobbler.com/2.0/';
const desktopCallbackOrigins = new Set([
  'http://127.0.0.1:1420',
  'http://127.0.0.1:28232',
]);

interface LastfmAuthorizationUrlOptions {
  apiKey?: string;
  desktop?: boolean;
  origin?: string;
}

interface LastfmTrackParams
  extends Record<string, string | number | undefined> {
  artist: string;
  track: string;
  album?: string;
  duration?: number;
  timestamp?: number;
}

type SignedLastfmParams = Record<string, string | number | undefined>;

const sign = (params: SignedLastfmParams): string => {
  const sortParamsKeys = Object.keys(params).sort();
  const sortedParams = sortParamsKeys.reduce<SignedLastfmParams>((acc, key) => {
    acc[key] = params[key];
    return acc;
  }, {});
  let signature = '';
  for (const [key, value] of Object.entries(sortedParams)) {
    signature += `${key}${value}`;
  }
  return md5(signature + apiSharedSecret).toString();
};

function getLastfmSessionKey(): string {
  const raw = localStorage.getItem('lastfm');
  if (!raw) return '';
  try {
    const parsed: unknown = JSON.parse(raw);
    return typeof parsed === 'object' &&
      parsed !== null &&
      'key' in parsed &&
      typeof parsed.key === 'string'
      ? parsed.key
      : '';
  } catch {
    return '';
  }
}

export function buildLastfmAuthorizationUrl({
  apiKey: authorizationApiKey = apiKey,
  desktop = isDesktopRuntime,
  origin = window.location.origin,
}: LastfmAuthorizationUrlOptions = {}): string {
  const callback = new URL(origin);
  if (desktop && !desktopCallbackOrigins.has(callback.origin)) {
    throw new Error('Last.fm desktop callback origin is not allowed');
  }
  callback.pathname = desktop ? '/' : '/lastfm/callback';
  callback.search = '';
  callback.hash = desktop ? '/lastfm/callback' : '';

  const authorization = new URL('https://www.last.fm/api/auth/');
  authorization.searchParams.set('api_key', authorizationApiKey);
  authorization.searchParams.set('cb', callback.href);
  return authorization.href;
}

export function readLastfmCallbackToken({
  search,
  hash,
}: Pick<Location, 'search' | 'hash'>): string | null {
  const queryToken = new URLSearchParams(search).get('token');
  if (queryToken) return queryToken;
  const hashQuery = hash.indexOf('?');
  return hashQuery === -1
    ? null
    : new URLSearchParams(hash.slice(hashQuery + 1)).get('token');
}

export function auth(): void {
  window.open(
    buildLastfmAuthorizationUrl({ desktop: false }),
    '_blank',
    'noopener,noreferrer'
  );
}

export function authGetSession(
  token: string
): Promise<AxiosResponse<{ session: UnknownRecord & { key: string } }>> {
  const signature = md5(
    `api_key${apiKey}methodauth.getSessiontoken${token}${apiSharedSecret}`
  ).toString();
  return axios
    .request<unknown>({
      url,
      method: 'GET',
      params: {
        method: 'auth.getSession',
        format: 'json',
        api_key: apiKey,
        api_sig: signature,
        token,
      },
    })
    .then(response => {
      const context = { url: 'https://ws.audioscrobbler.com/2.0/' };
      const data = decodeRecord(response.data, context);
      const session = decodeRecord(data['session'], context, '$.session');
      return {
        ...response,
        data: {
          session: {
            ...session,
            key: decodeString(session['key'], context, '$.session.key'),
          },
        },
      };
    });
}

export function trackUpdateNowPlaying(params: LastfmTrackParams) {
  const signedParams: SignedLastfmParams = {
    ...params,
    api_key: apiKey,
    method: 'track.updateNowPlaying',
    sk: getLastfmSessionKey(),
  };
  const signature = sign(signedParams);

  return axios.request<unknown>({
    url,
    method: 'POST',
    params: {
      ...signedParams,
      api_sig: signature,
      format: 'json',
    },
  });
}

export function trackScrobble(params: LastfmTrackParams) {
  const signedParams: SignedLastfmParams = {
    ...params,
    api_key: apiKey,
    method: 'track.scrobble',
    sk: getLastfmSessionKey(),
  };
  const signature = sign(signedParams);

  return axios.request<unknown>({
    url,
    method: 'POST',
    params: {
      ...signedParams,
      api_sig: signature,
      format: 'json',
    },
  });
}
