/**
 * Converts streamed FLAC to WAV with afconvert and serves byte ranges for precise seeking.
 * This keeps peak memory near 10 MB instead of roughly 300 MB in the renderer.
 * Temporary disk usage is capped at one track.
 */
import path from 'node:path';
import os from 'node:os';
import fs from 'node:fs';
import fsp from 'node:fs/promises';
import { spawn } from 'node:child_process';
import type { Application, Request, Response } from 'express';

export const PRECISE_WAV_DIR = path.join(
  os.tmpdir(),
  'yesplaymusic-precise-wav'
);
const AFCONVERT_TIMEOUT_MS = 20000;

export function afconvertDataFormat(bitsPerSample: unknown): string {
  const bits = Number(bitsPerSample);
  // Preserve integer depth; Float32 cannot exactly represent 25-32 bit samples.
  if (!Number.isFinite(bits) || bits <= 16) return 'LEI16';
  if (bits <= 24) return 'LEI24';
  return 'LEI32';
}

export function preciseWavFileName(trackId: unknown): string | null {
  const id = String(trackId);
  return /^[0-9]{1,20}$/.test(id) ? `${id}.wav` : null;
}

export async function sweepPreciseWavDir(
  dir: string,
  keepFileNames: string[] = []
): Promise<void> {
  const keep = new Set(keepFileNames);
  const entries = await fsp.readdir(dir).catch((): string[] => []);
  await Promise.all(
    entries
      .filter(entry => !keep.has(entry))
      .map(entry =>
        fsp.rm(path.join(dir, entry), { force: true }).catch(() => {})
      )
  );
}

function runAfconvert(
  flacPath: string,
  wavPath: string,
  dataFormat: string
): Promise<void> {
  return new Promise<void>((resolve, reject) => {
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

// Bound temporary disk use above a typical lossless track.
const MAX_UPLOAD_BYTES = 512 * 1024 * 1024;
const UPLOAD_TIMEOUT_MS = 60000;

function persistUpload(
  request: Request,
  filePath: string,
  { maxBytes, timeoutMs }: { maxBytes: number; timeoutMs: number }
): Promise<void> {
  return new Promise<void>((resolve, reject) => {
    const out = fs.createWriteStream(filePath);
    let received = 0;
    let settled = false;
    const fail = (error: Error) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      request.unpipe(out);
      out.destroy();
      request.destroy();
      reject(error);
    };
    const timer = setTimeout(() => fail(new Error('上传超时')), timeoutMs);
    request.on('data', (chunk: Buffer | string) => {
      received += chunk.length;
      if (received > maxBytes) fail(new Error('超出上传大小上限'));
    });
    request.once('error', fail);
    out.once('error', fail);
    out.once('finish', () => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      resolve();
    });
    request.pipe(out);
  });
}

export function installPreciseWavRoutes(
  app: Application,
  {
    tempDir = PRECISE_WAV_DIR,
    convert = runAfconvert,
    platform = process.platform,
    maxUploadBytes = MAX_UPLOAD_BYTES,
    uploadTimeoutMs = UPLOAD_TIMEOUT_MS,
  }: {
    tempDir?: string;
    convert?: (
      flacPath: string,
      wavPath: string,
      dataFormat: string
    ) => Promise<void>;
    platform?: NodeJS.Platform;
    maxUploadBytes?: number;
    uploadTimeoutMs?: number;
  } = {}
): void {
  // Remove temporary files left by the previous session.
  void sweepPreciseWavDir(tempDir);
  // Serialize cleanup and writes even though the renderer sends one job at a time.
  let converting = false;

  app.post(
    '/precise-wav/:trackId',
    async (request: Request, response: Response) => {
      if (platform !== 'darwin') {
        // Return 501 where afconvert is unavailable so the renderer uses its fallback.
        response.status(501).send({ message: '当前平台不提供原生 WAV 转换' });
        return;
      }
      const wavName = preciseWavFileName(request.params['trackId']);
      if (!wavName) {
        response.status(400).send({ message: '无效的歌曲 ID' });
        return;
      }
      const declaredBytes = Number(request.headers['content-length']);
      if (declaredBytes > maxUploadBytes) {
        response.status(413).send({ message: '文件超出大小上限' });
        return;
      }
      if (converting) {
        response.status(429).send({ message: '已有转换在进行中' });
        return;
      }
      converting = true;
      const flacPath = path.join(tempDir, wavName.replace(/\.wav$/, '.flac'));
      const wavPath = path.join(tempDir, wavName);
      try {
        await fsp.mkdir(tempDir, { recursive: true });
        // Keep only the current track on disk.
        await sweepPreciseWavDir(tempDir);
        // Stream the request to disk instead of buffering the full FLAC.
        await persistUpload(request, flacPath, {
          maxBytes: maxUploadBytes,
          timeoutMs: uploadTimeoutMs,
        });
        await convert(
          flacPath,
          wavPath,
          afconvertDataFormat(request.query['bits'])
        );
        // Finish cleanup before responding to avoid observable stale input files.
        await fsp.rm(flacPath, { force: true });
        response.send({ url: `/precise-wav/${wavName}` });
      } catch (error: unknown) {
        const message = error instanceof Error ? error.message : String(error);
        console.warn(`[sidecar][precise-wav] 转换失败：${message}`);
        // Remove partial files before responding because clients fall back immediately.
        await Promise.all([
          fsp.rm(flacPath, { force: true }).catch(() => {}),
          fsp.rm(wavPath, { force: true }).catch(() => {}),
        ]);
        if (!response.headersSent) {
          response.status(500).send({ message: '转换失败' });
        }
      } finally {
        converting = false;
        await fsp.rm(flacPath, { force: true }).catch(() => {});
      }
    }
  );

  // Release temporary disk space when playback leaves the track.
  app.delete('/precise-wav', async (_request: Request, response: Response) => {
    await sweepPreciseWavDir(tempDir);
    response.status(204).end();
  });

  app.get('/precise-wav/:fileName', (request: Request, response: Response) => {
    const requested = path.basename(request.params['fileName'] ?? '');
    if (!/^[0-9]{1,20}\.wav$/.test(requested)) {
      response.status(400).end();
      return;
    }
    // sendFile provides ranges, and AVPlayer can seek WAV by byte offset.
    response.sendFile(path.join(tempDir, requested), (error: Error) => {
      if (error && !response.headersSent) response.status(404).end();
    });
  });
}
