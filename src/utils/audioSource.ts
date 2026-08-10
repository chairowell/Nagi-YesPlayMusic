const AUDIO_SOURCE_ORIGINS = ['cache', 'netease', 'unm'] as const;
export type AudioSourceOrigin = (typeof AUDIO_SOURCE_ORIGINS)[number];

const FORMAT_ALIASES: Record<string, string> = {
  mpeg: 'mp3',
  mpga: 'mp3',
  'x-mp3': 'mp3',
  'x-flac': 'flac',
  'x-m4a': 'm4a',
  mp4: 'm4a',
};

const AUDIO_MIME_TYPES: Record<string, string> = {
  mp3: 'audio/mpeg',
  flac: 'audio/flac',
  m4a: 'audio/mp4',
  aac: 'audio/aac',
  ogg: 'audio/ogg',
  opus: 'audio/ogg; codecs=opus',
  wav: 'audio/wav',
  webm: 'audio/webm',
};

export interface AudioSource<TUrl = string> extends Record<string, unknown> {
  url: TUrl;
  origin: string;
  format: string | null;
  mimeType: string | null;
  provider?: string;
  excludedProviders?: string[];
  cacheAfterLoad?: (() => unknown | Promise<unknown>) | null;
}

export interface RemoteAudioSourceOptions {
  origin?: string;
  format?: unknown;
  fallbackFormat?: unknown;
  provider?: string;
  excludedProviders?: string[];
  cacheAfterLoad?: (() => unknown | Promise<unknown>) | null;
}

export function normalizeAudioFormat(format: unknown): string | null {
  if (!format || typeof format !== 'string') return null;
  const normalized = format.toLowerCase().replace(/^audio\//, '');
  return FORMAT_ALIASES[normalized] || normalized;
}

export function sniffAudioFormat(data: unknown): string | null {
  const bytes =
    data instanceof Uint8Array
      ? data
      : data instanceof ArrayBuffer
      ? new Uint8Array(data)
      : ArrayBuffer.isView(data)
      ? new Uint8Array(data.buffer, data.byteOffset, data.byteLength)
      : null;
  if (!bytes || bytes.length < 4) return null;

  if (
    bytes[0] === 0x66 &&
    bytes[1] === 0x4c &&
    bytes[2] === 0x61 &&
    bytes[3] === 0x43
  ) {
    return 'flac';
  }
  if (bytes[0] === 0x49 && bytes[1] === 0x44 && bytes[2] === 0x33) {
    return 'mp3';
  }
  if (
    bytes[0] === 0x4f &&
    bytes[1] === 0x67 &&
    bytes[2] === 0x67 &&
    bytes[3] === 0x53
  ) {
    const header = new TextDecoder().decode(bytes.subarray(0, 64));
    return header.includes('OpusHead') ? 'opus' : 'ogg';
  }
  if (
    bytes.length >= 12 &&
    bytes[0] === 0x52 &&
    bytes[1] === 0x49 &&
    bytes[2] === 0x46 &&
    bytes[3] === 0x46 &&
    bytes[8] === 0x57 &&
    bytes[9] === 0x41 &&
    bytes[10] === 0x56 &&
    bytes[11] === 0x45
  ) {
    return 'wav';
  }
  if (
    bytes[0] === 0x1a &&
    bytes[1] === 0x45 &&
    bytes[2] === 0xdf &&
    bytes[3] === 0xa3
  ) {
    return 'webm';
  }
  if (
    bytes.length >= 8 &&
    bytes[4] === 0x66 &&
    bytes[5] === 0x74 &&
    bytes[6] === 0x79 &&
    bytes[7] === 0x70
  ) {
    return 'm4a';
  }
  if (bytes[0] === 0xff && (bytes[1]! & 0xf6) === 0xf0) return 'aac';
  if (bytes[0] === 0xff && (bytes[1]! & 0xe0) === 0xe0) return 'mp3';
  return null;
}

export function inferAudioFormatFromUrl(url: unknown): string | null {
  if (typeof url !== 'string') return null;
  const dataMime = /^data:audio\/([^;,]+)/i.exec(url)?.[1];
  if (dataMime) return normalizeAudioFormat(dataMime);
  const extension = /\.([a-z0-9]+)(?:$|[?#])/i.exec(url)?.[1];
  return normalizeAudioFormat(extension);
}

export function createBlobAudioSource<TUrl = string>(
  data: BlobPart,
  createObjectURL: (blob: Blob) => TUrl = blob =>
    URL.createObjectURL(blob) as TUrl,
  origin = 'cache'
): AudioSource<TUrl> {
  const format = sniffAudioFormat(data);
  const mimeType =
    (format && AUDIO_MIME_TYPES[format]) || 'application/octet-stream';
  const blob = new Blob([data], { type: mimeType });
  return {
    url: createObjectURL(blob),
    origin,
    format,
    mimeType,
  };
}

export function createRemoteAudioSource(
  url: string,
  options: RemoteAudioSourceOptions = {}
): AudioSource {
  const format =
    normalizeAudioFormat(options.format) ||
    inferAudioFormatFromUrl(url) ||
    normalizeAudioFormat(options.fallbackFormat);
  return {
    ...options,
    url,
    format,
    origin: options.origin ?? 'remote',
    mimeType: (format && AUDIO_MIME_TYPES[format]) || null,
  };
}

export function toHowlSourceOptions(source: AudioSource): {
  src: string[];
  format?: string;
} {
  const options: { src: string[]; format?: string } = { src: [source.url] };
  if (source.format) options.format = source.format;
  return options;
}

export function getAudioSourceOriginsAfter(
  origin: AudioSourceOrigin | null = null
): AudioSourceOrigin[] {
  if (origin === null) return [...AUDIO_SOURCE_ORIGINS];
  const index = AUDIO_SOURCE_ORIGINS.indexOf(origin);
  return index < 0 ? [] : AUDIO_SOURCE_ORIGINS.slice(index + 1);
}

export function isAudioSourceOrigin(value: string): value is AudioSourceOrigin {
  return AUDIO_SOURCE_ORIGINS.includes(value as AudioSourceOrigin);
}

export async function resolveAudioSource(
  resolvers: Partial<
    Record<
      AudioSourceOrigin,
      () => AudioSource | null | Promise<AudioSource | null>
    >
  >,
  afterOrigin: AudioSourceOrigin | null = null,
  onError: (origin: AudioSourceOrigin, error: unknown) => void = () => {}
): Promise<AudioSource | null> {
  for (const origin of getAudioSourceOriginsAfter(afterOrigin)) {
    const resolve = resolvers[origin];
    if (!resolve) continue;
    try {
      const source = await resolve();
      if (source) return source;
    } catch (error) {
      onError(origin, error);
    }
  }
  return null;
}

export async function discardFailedCache<TId, TResult>(
  deleteSource: (trackID: TId) => TResult | Promise<TResult>,
  trackID: TId,
  onError: (error: unknown) => void
): Promise<TResult | false> {
  try {
    return await deleteSource(trackID);
  } catch (error) {
    onError(error);
    return false;
  }
}
