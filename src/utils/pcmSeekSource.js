/**
 * WebKit/AVPlayer 对 FLAC 的 seek 按字节估算落点：实际声音落在请求位置前
 * 数秒，currentTime 却按请求值计时，歌词时钟随之整体领先。WAV 是恒定码率
 * PCM，时间到字节是纯算术，AVPlayer 对它采样级精确。因此拖拽时把缓存的
 * FLAC 离线解码重打包成 float32 WAV（解码已实测逐样本无损），播放仍走
 * 系统媒体栈，不引入 Web Audio 的实时输出路径。
 */

export function parseFlacStreamInfo(arrayBuffer) {
  const bytes =
    arrayBuffer instanceof Uint8Array ? arrayBuffer : new Uint8Array(arrayBuffer);
  // 'fLaC' + 块头 4B + STREAMINFO 的采样率字段止于第 21 字节
  if (bytes.length < 22) return null;
  if (
    bytes[0] !== 0x66 ||
    bytes[1] !== 0x4c ||
    bytes[2] !== 0x61 ||
    bytes[3] !== 0x43
  ) {
    return null;
  }
  // 规范保证 STREAMINFO 是第一个元数据块；采样率在其第 10 字节起占 20 bits，
  // 后接 3 bits 声道数-1 与 5 bits 位深-1
  const offset = 8 + 10;
  const sampleRate =
    (bytes[offset] << 12) | (bytes[offset + 1] << 4) | (bytes[offset + 2] >> 4);
  const channels = ((bytes[offset + 2] >> 1) & 0x07) + 1;
  const bitsPerSample =
    (((bytes[offset + 2] & 0x01) << 4) | (bytes[offset + 3] >> 4)) + 1;
  if (sampleRate <= 0 || sampleRate > 384000) return null;
  if (bitsPerSample < 4 || bitsPerSample > 32) return null;
  return { sampleRate, channels, bitsPerSample };
}

/**
 * 请求 sidecar 用原生 afconvert 把整曲缓存转成临时 WAV 并返回 Range URL。
 * sidecar 不可达（Electron、dev server）或转换失败时返回 null，调用方
 * 退回渲染进程内的 decodeFlacToWavBlob。任何异常都不外抛。
 */
export async function requestPreciseWavURL(
  trackId,
  arrayBuffer,
  bitsPerSample,
  fetchImpl = typeof fetch === 'function' ? fetch : null
) {
  if (!fetchImpl) return null;
  try {
    const bits = Number(bitsPerSample) || 16;
    const response = await fetchImpl(
      `/precise-wav/${trackId}?bits=${bits}`,
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/octet-stream' },
        body: arrayBuffer,
      }
    );
    if (!response.ok) return null;
    const payload = await response.json();
    return typeof payload?.url === 'string' ? payload.url : null;
  } catch {
    return null;
  }
}

export function buildFloat32WavBlob(audioBuffer) {
  const channels = audioBuffer.numberOfChannels;
  const frames = audioBuffer.length;
  const rate = audioBuffer.sampleRate;
  const dataBytes = frames * channels * 4;

  const header = new ArrayBuffer(58);
  const view = new DataView(header);
  const writeTag = (offset, tag) => {
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
  // 非 PCM 格式规范要求 fact 块
  writeTag(38, 'fact');
  view.setUint32(42, 4, true);
  view.setUint32(46, frames, true);
  writeTag(50, 'data');
  view.setUint32(54, dataBytes, true);

  // Float32Array 要求 4 字节对齐，58 字节的头必须与数据分开两个 buffer
  const data = new ArrayBuffer(dataBytes);
  const interleaved = new Float32Array(data);
  const channelData = new Float32Array(frames);
  for (let channel = 0; channel < channels; channel++) {
    audioBuffer.copyFromChannel(channelData, channel);
    for (let i = 0; i < frames; i++) {
      interleaved[i * channels + channel] = channelData[i];
    }
  }
  return new Blob([header, data], { type: 'audio/wav' });
}

/**
 * 把 FLAC 字节离线解码为 float32 WAV Blob。按 STREAMINFO 的原生采样率
 * 解码，decodeAudioData 不做重采样，样本与源文件一致。
 */
export async function decodeFlacToWavBlob(
  arrayBuffer,
  createOfflineContext = (channels, sampleRate) =>
    new (window.OfflineAudioContext || window.webkitOfflineAudioContext)(
      channels,
      1,
      sampleRate
    )
) {
  const info = parseFlacStreamInfo(arrayBuffer);
  if (!info) return null;
  const context = createOfflineContext(info.channels, info.sampleRate);
  const audioBuffer = await context.decodeAudioData(arrayBuffer);
  return buildFloat32WavBlob(audioBuffer);
}
