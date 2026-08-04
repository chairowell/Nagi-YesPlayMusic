const AUDIO_SOURCE_ORIGINS = ['cache', 'netease', 'unm'];

const FORMAT_ALIASES = {
  mpeg: 'mp3',
  mpga: 'mp3',
  'x-mp3': 'mp3',
  'x-flac': 'flac',
  'x-m4a': 'm4a',
  mp4: 'm4a',
};

const AUDIO_MIME_TYPES = {
  mp3: 'audio/mpeg',
  flac: 'audio/flac',
  m4a: 'audio/mp4',
  aac: 'audio/aac',
  ogg: 'audio/ogg',
  opus: 'audio/ogg; codecs=opus',
  wav: 'audio/wav',
  webm: 'audio/webm',
};

export function normalizeAudioFormat(format) {
  if (!format || typeof format !== 'string') return null;
  const normalized = format.toLowerCase().replace(/^audio\//, '');
  return FORMAT_ALIASES[normalized] || normalized;
}

export function sniffAudioFormat(data) {
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
  if (bytes[0] === 0xff && (bytes[1] & 0xf6) === 0xf0) return 'aac';
  if (bytes[0] === 0xff && (bytes[1] & 0xe0) === 0xe0) return 'mp3';
  return null;
}

export function inferAudioFormatFromUrl(url) {
  if (typeof url !== 'string') return null;
  const dataMime = /^data:audio\/([^;,]+)/i.exec(url)?.[1];
  if (dataMime) return normalizeAudioFormat(dataMime);
  const extension = /\.([a-z0-9]+)(?:$|[?#])/i.exec(url)?.[1];
  return normalizeAudioFormat(extension);
}

export function createBlobAudioSource(
  data,
  createObjectURL = blob => URL.createObjectURL(blob),
  origin = 'cache'
) {
  const format = sniffAudioFormat(data);
  const mimeType = AUDIO_MIME_TYPES[format] || 'application/octet-stream';
  const blob = new Blob([data], { type: mimeType });
  return {
    url: createObjectURL(blob),
    origin,
    format,
    mimeType,
  };
}

export function createRemoteAudioSource(url, options = {}) {
  const format =
    normalizeAudioFormat(options.format) ||
    inferAudioFormatFromUrl(url) ||
    normalizeAudioFormat(options.fallbackFormat);
  return {
    ...options,
    url,
    format,
    mimeType: AUDIO_MIME_TYPES[format] || null,
  };
}

export function toHowlSourceOptions(source) {
  const options = { src: [source.url] };
  if (source.format) options.format = source.format;
  return options;
}

export function getAudioSourceOriginsAfter(origin = null) {
  if (origin === null) return [...AUDIO_SOURCE_ORIGINS];
  const index = AUDIO_SOURCE_ORIGINS.indexOf(origin);
  return index < 0 ? [] : AUDIO_SOURCE_ORIGINS.slice(index + 1);
}

export async function resolveAudioSource(
  resolvers,
  afterOrigin = null,
  onError = () => {}
) {
  for (const origin of getAudioSourceOriginsAfter(afterOrigin)) {
    try {
      const source = await resolvers[origin]();
      if (source) return source;
    } catch (error) {
      onError(origin, error);
    }
  }
  return null;
}

export async function discardFailedCache(deleteSource, trackID, onError) {
  try {
    return await deleteSource(trackID);
  } catch (error) {
    onError(error);
    return false;
  }
}
