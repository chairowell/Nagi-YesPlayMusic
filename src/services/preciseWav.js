/**
 * FLAC 拖拽精确 seek 的低内存链路：渲染端把缓存 FLAC 流式 POST 过来，
 * 这里落盘后用 macOS 自带的 afconvert 流式转成 WAV（实测峰值 RSS ~10MB、
 * 300 秒歌曲 0.2 秒内完成），再以 HTTP Range 按需服务。相比渲染进程里
 * decodeAudioData + 重打包（瞬时 ~300MB），内存开销可忽略；代价是临时
 * 磁盘占用一首歌（~85MB），由清扫策略封顶。
 */
import path from 'node:path';
import os from 'node:os';
import fs from 'node:fs';
import fsp from 'node:fs/promises';
import { spawn } from 'node:child_process';
import { pipeline } from 'node:stream/promises';

export const PRECISE_WAV_DIR = path.join(
  os.tmpdir(),
  'yesplaymusic-precise-wav'
);
const AFCONVERT_TIMEOUT_MS = 20000;

export function afconvertDataFormat(bitsPerSample) {
  const bits = Number(bitsPerSample);
  // WAV 保持源位深即为无损；异常输入回退 16-bit（网易云最常见）
  if (!Number.isFinite(bits) || bits <= 16) return 'LEI16';
  if (bits <= 24) return 'LEI24';
  return 'LEF32';
}

export function preciseWavFileName(trackId) {
  const id = String(trackId);
  return /^[0-9]{1,20}$/.test(id) ? `${id}.wav` : null;
}

export async function sweepPreciseWavDir(dir, keepFileNames = []) {
  const keep = new Set(keepFileNames);
  const entries = await fsp.readdir(dir).catch(() => []);
  await Promise.all(
    entries
      .filter(entry => !keep.has(entry))
      .map(entry => fsp.rm(path.join(dir, entry), { force: true }).catch(() => {}))
  );
}

function runAfconvert(flacPath, wavPath, dataFormat) {
  return new Promise((resolve, reject) => {
    const child = spawn(
      'afconvert',
      ['-f', 'WAVE', '-d', dataFormat, flacPath, wavPath],
      { stdio: ['ignore', 'ignore', 'pipe'] }
    );
    let stderr = '';
    child.stderr.on('data', chunk => {
      stderr += chunk;
    });
    const timer = setTimeout(() => child.kill('SIGKILL'), AFCONVERT_TIMEOUT_MS);
    child.once('error', error => {
      clearTimeout(timer);
      reject(error);
    });
    child.once('close', code => {
      clearTimeout(timer);
      if (code === 0) resolve();
      else reject(new Error(`afconvert exit ${code}: ${stderr.slice(0, 200)}`));
    });
  });
}

export function installPreciseWavRoutes(
  app,
  { tempDir = PRECISE_WAV_DIR, convert = runAfconvert } = {}
) {
  // 上次会话的残留文件在启动时清掉
  void sweepPreciseWavDir(tempDir);

  app.post('/precise-wav/:trackId', async (request, response) => {
    const wavName = preciseWavFileName(request.params.trackId);
    if (!wavName) {
      response.status(400).send({ message: '无效的歌曲 ID' });
      return;
    }
    const flacPath = path.join(tempDir, wavName.replace(/\.wav$/, '.flac'));
    const wavPath = path.join(tempDir, wavName);
    try {
      await fsp.mkdir(tempDir, { recursive: true });
      // 只保留当前这首，临时磁盘占用封顶一首歌
      await sweepPreciseWavDir(tempDir);
      // 请求体流式落盘，整首 FLAC 不进内存
      await pipeline(request, fs.createWriteStream(flacPath));
      await convert(flacPath, wavPath, afconvertDataFormat(request.query.bits));
      response.send({ url: `/precise-wav/${wavName}` });
    } catch (error) {
      console.warn(`[sidecar][precise-wav] 转换失败：${error.message}`);
      response.status(500).send({ message: '转换失败' });
    } finally {
      await fsp.rm(flacPath, { force: true }).catch(() => {});
    }
  });

  app.get('/precise-wav/:fileName', (request, response) => {
    const requested = path.basename(request.params.fileName);
    if (!/^[0-9]{1,20}\.wav$/.test(requested)) {
      response.status(400).end();
      return;
    }
    // res.sendFile 自带 Range 支持；AVPlayer 对 WAV 的 seek 是纯字节算术
    response.sendFile(path.join(tempDir, requested), error => {
      if (error && !response.headersSent) response.status(404).end();
    });
  });
}
