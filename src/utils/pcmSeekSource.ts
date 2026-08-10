/**
 * AVPlayer estimates FLAC seeks by byte position, desynchronizing audio and
 * currentTime. Repack cached FLAC as constant-rate float32 WAV for precise seeks
 * while keeping playback on the system media stack.
 */

export interface FlacStreamInfo {
  sampleRate: number;
  channels: number;
  bitsPerSample: number;
}

interface FetchResponseLike {
  ok: boolean;
  json?: () => Promise<unknown>;
}

type FetchLike = (
  input: string,
  init: RequestInit
) => Promise<FetchResponseLike>;

interface AudioBufferLike {
  numberOfChannels: number;
  length: number;
  sampleRate: number;
  copyFromChannel(destination: Float32Array, channelNumber: number): void;
}

interface OfflineAudioContextLike {
  decodeAudioData(audioData: ArrayBuffer): Promise<AudioBuffer>;
}

export function parseFlacStreamInfo(
  arrayBuffer: ArrayBuffer | Uint8Array
): FlacStreamInfo | null {
  const bytes =
    arrayBuffer instanceof Uint8Array
      ? arrayBuffer
      : new Uint8Array(arrayBuffer);
  // 'fLaC', a 4-byte block header, and STREAMINFO through its sample-rate field.
  if (bytes.length < 22) return null;
  if (
    bytes[0] !== 0x66 ||
    bytes[1] !== 0x4c ||
    bytes[2] !== 0x61 ||
    bytes[3] !== 0x43
  ) {
    return null;
  }
  // STREAMINFO stores sample rate, channels - 1, and bit depth - 1 contiguously.
  const offset = 8 + 10;
  const sampleRate =
    (bytes[offset]! << 12) |
    (bytes[offset + 1]! << 4) |
    (bytes[offset + 2]! >> 4);
  const channels = ((bytes[offset + 2]! >> 1) & 0x07) + 1;
  const bitsPerSample =
    (((bytes[offset + 2]! & 0x01) << 4) | (bytes[offset + 3]! >> 4)) + 1;
  if (sampleRate <= 0 || sampleRate > 384000) return null;
  if (bitsPerSample < 4 || bitsPerSample > 32) return null;
  return { sampleRate, channels, bitsPerSample };
}

// Remove the temporary WAV when switching tracks.
export function discardPreciseWav(
  fetchImpl: FetchLike | null = typeof fetch === 'function' ? fetch : null
): Promise<boolean> {
  if (!fetchImpl) return Promise.resolve(false);
  return fetchImpl('/precise-wav', { method: 'DELETE' })
    .then(response => response.ok)
    .catch(() => false);
}

export async function requestPreciseWavURL(
  trackId: number,
  arrayBuffer: ArrayBuffer,
  bitsPerSample: number | undefined,
  fetchImpl: FetchLike | null = typeof fetch === 'function' ? fetch : null
): Promise<string | null> {
  if (!fetchImpl) return null;
  try {
    const bits = Number(bitsPerSample) || 16;
    const response = await fetchImpl(`/precise-wav/${trackId}?bits=${bits}`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/octet-stream' },
      body: arrayBuffer,
    });
    if (!response.ok) return null;
    const payload = await response.json?.();
    return typeof payload === 'object' &&
      payload !== null &&
      'url' in payload &&
      typeof payload.url === 'string'
      ? payload.url
      : null;
  } catch {
    return null;
  }
}

export function buildFloat32WavBlob(audioBuffer: AudioBufferLike): Blob {
  const channels = audioBuffer.numberOfChannels;
  const frames = audioBuffer.length;
  const rate = audioBuffer.sampleRate;
  const dataBytes = frames * channels * 4;

  const header = new ArrayBuffer(58);
  const view = new DataView(header);
  const writeTag = (offset: number, tag: string) => {
    for (let i = 0; i < tag.length; i++) {
      view.setUint8(offset + i, tag.charCodeAt(i));
    }
  };
  writeTag(0, 'RIFF');
  view.setUint32(4, 50 + dataBytes, true);
  writeTag(8, 'WAVE');
  writeTag(12, 'fmt ');
  view.setUint32(16, 18, true);
  view.setUint16(20, 3, true); // WAVE_FORMAT_IEEE_FLOAT
  view.setUint16(22, channels, true);
  view.setUint32(24, rate, true);
  view.setUint32(28, rate * channels * 4, true);
  view.setUint16(32, channels * 4, true);
  view.setUint16(34, 32, true);
  view.setUint16(36, 0, true); // cbSize
  // Non-PCM WAV requires a fact chunk.
  writeTag(38, 'fact');
  view.setUint32(42, 4, true);
  view.setUint32(46, frames, true);
  writeTag(50, 'data');
  view.setUint32(54, dataBytes, true);

  // Keep the 58-byte header separate to preserve Float32Array alignment.
  const data = new ArrayBuffer(dataBytes);
  const interleaved = new Float32Array(data);
  const channelData = new Float32Array(frames);
  for (let channel = 0; channel < channels; channel++) {
    audioBuffer.copyFromChannel(channelData, channel);
    for (let i = 0; i < frames; i++) {
      interleaved[i * channels + channel] = channelData[i] ?? 0;
    }
  }
  return new Blob([header, data], { type: 'audio/wav' });
}

/**
 * Decode FLAC into a float32 WAV Blob at its native STREAMINFO sample rate.
 */
export async function decodeFlacToWavBlob(
  arrayBuffer: ArrayBuffer,
  createOfflineContext: (
    channels: number,
    sampleRate: number
  ) => OfflineAudioContextLike = (channels, sampleRate) =>
    new (window.OfflineAudioContext || window.webkitOfflineAudioContext)(
      channels,
      1,
      sampleRate
    )
): Promise<Blob | null> {
  const info = parseFlacStreamInfo(arrayBuffer);
  if (!info) return null;
  const context = createOfflineContext(info.channels, info.sampleRate);
  const audioBuffer = await context.decodeAudioData(arrayBuffer);
  return buildFloat32WavBlob(audioBuffer);
}
