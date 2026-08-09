import { afterAll, beforeAll, describe, expect, test } from 'bun:test';
import express from 'express';
import { mkdtempSync, writeFileSync } from 'node:fs';
import fsp from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import {
  afconvertDataFormat,
  preciseWavFileName,
  sweepPreciseWavDir,
  installPreciseWavRoutes,
} from '../src/services/preciseWav';

describe('afconvert 参数与文件名约束', () => {
  test('位深映射保持无损：16→LEI16、24→LEI24、32→LEI32、异常回退 LEI16', () => {
    expect(afconvertDataFormat(16)).toBe('LEI16');
    expect(afconvertDataFormat(24)).toBe('LEI24');
    // Float32 只有 24-bit 整数精度，25~32-bit 必须走 LEI32 才无损
    expect(afconvertDataFormat(32)).toBe('LEI32');
    expect(afconvertDataFormat(undefined)).toBe('LEI16');
    expect(afconvertDataFormat('嗯')).toBe('LEI16');
  });

  test('歌曲 ID 只接受纯数字，路径穿越直接拒绝', () => {
    expect(preciseWavFileName(1387548496)).toBe('1387548496.wav');
    expect(preciseWavFileName('../../etc/passwd')).toBeNull();
    expect(preciseWavFileName('abc')).toBeNull();
  });
});

describe('precise-wav 路由', () => {
  let server;
  let base;
  let tempDir;
  let convertCalls;

  beforeAll(async () => {
    tempDir = mkdtempSync(path.join(os.tmpdir(), 'precise-wav-test-'));
    convertCalls = [];
    const app = express();
    installPreciseWavRoutes(app, {
      tempDir,
      platform: 'darwin',
      convert: async (flacPath, wavPath, dataFormat) => {
        convertCalls.push({ flacPath, wavPath, dataFormat });
        if (dataFormat === 'LEI32') {
          // 模拟 afconvert 中途失败：半成品 WAV 已经写了一半
          await fsp.writeFile(wavPath, '半成品');
          throw new Error('模拟转换失败');
        }
        const flacBytes = await fsp.readFile(flacPath);
        // 假转换器：证明收到的就是 POST 的字节
        await fsp.writeFile(
          wavPath,
          Buffer.concat([Buffer.from('RIFF'), flacBytes])
        );
      },
    });
    server = await new Promise(resolve => {
      const s = app.listen(0, '127.0.0.1', () => resolve(s));
    });
    base = `http://127.0.0.1:${server.address().port}`;
  });

  afterAll(async () => {
    await new Promise(resolve => server.close(resolve));
    await fsp.rm(tempDir, { recursive: true, force: true });
  });

  test('POST 流式落盘转换，GET 全量与 Range 均可读', async () => {
    const body = new Uint8Array([1, 2, 3, 4, 5, 6, 7, 8]);
    const post = await fetch(`${base}/precise-wav/42?bits=24`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/octet-stream' },
      body,
    });
    expect(post.status).toBe(200);
    expect((await post.json()).url).toBe('/precise-wav/42.wav');
    expect(convertCalls[0].dataFormat).toBe('LEI24');

    const full = await fetch(`${base}/precise-wav/42.wav`);
    expect(full.status).toBe(200);
    const fullBytes = new Uint8Array(await full.arrayBuffer());
    expect(Array.from(fullBytes.slice(4))).toEqual([1, 2, 3, 4, 5, 6, 7, 8]);

    const partial = await fetch(`${base}/precise-wav/42.wav`, {
      headers: { Range: 'bytes=4-7' },
    });
    expect(partial.status).toBe(206);
    expect(Array.from(new Uint8Array(await partial.arrayBuffer()))).toEqual([
      1, 2, 3, 4,
    ]);
    // 中间产物 .flac 转换完就删
    expect(await fsp.readdir(tempDir)).toEqual(['42.wav']);
  });

  test('新歌的转换会清掉上一首的临时 WAV，磁盘占用封顶一首', async () => {
    const post = await fetch(`${base}/precise-wav/43?bits=16`, {
      method: 'POST',
      body: new Uint8Array([9]),
    });
    expect(post.status).toBe(200);
    expect(await fsp.readdir(tempDir)).toEqual(['43.wav']);
    expect((await fetch(`${base}/precise-wav/42.wav`)).status).toBe(404);
  });

  test('非法 ID 与转换失败分别返回 400/500，失败不留 .flac 也不留半成品 WAV', async () => {
    expect(
      (await fetch(`${base}/precise-wav/..%2Fevil`, { method: 'POST' })).status
    ).toBe(400);
    const failed = await fetch(`${base}/precise-wav/44?bits=32`, {
      method: 'POST',
      body: new Uint8Array([1]),
    });
    expect(failed.status).toBe(500);
    const remaining = await fsp.readdir(tempDir);
    expect(remaining.includes('44.flac')).toBe(false);
    expect(remaining.includes('44.wav')).toBe(false);
  });

  test('超过大小上限的上传直接 413 拒绝', async () => {
    const tiny = mkdtempSync(path.join(os.tmpdir(), 'precise-wav-tiny-'));
    const app = express();
    installPreciseWavRoutes(app, {
      tempDir: tiny,
      platform: 'darwin',
      convert: async () => {},
      maxUploadBytes: 4,
    });
    const s = await new Promise(resolve => {
      const inner = app.listen(0, '127.0.0.1', () => resolve(inner));
    });
    const oversize = await fetch(
      `http://127.0.0.1:${s.address().port}/precise-wav/45`,
      { method: 'POST', body: new Uint8Array(8) }
    );
    expect(oversize.status).toBe(413);
    await new Promise(resolve => s.close(resolve));
    await fsp.rm(tiny, { recursive: true, force: true });
  });

  test('DELETE 立即清空临时目录（切歌清扫）', async () => {
    await fetch(`${base}/precise-wav/46?bits=16`, {
      method: 'POST',
      body: new Uint8Array([1]),
    });
    expect((await fsp.readdir(tempDir)).length).toBeGreaterThan(0);
    const wipe = await fetch(`${base}/precise-wav`, { method: 'DELETE' });
    expect(wipe.status).toBe(204);
    expect(await fsp.readdir(tempDir)).toEqual([]);
  });

  test('启动清扫会清空历史残留', async () => {
    writeFileSync(path.join(tempDir, 'stale.wav'), 'x');
    await sweepPreciseWavDir(tempDir);
    expect(await fsp.readdir(tempDir)).toEqual([]);
  });

  test('Windows/Linux 明确返回 501，让播放器使用已有回退路径', async () => {
    const app = express();
    installPreciseWavRoutes(app, { platform: 'win32' });
    const s = await new Promise(resolve => {
      const inner = app.listen(0, '127.0.0.1', () => resolve(inner));
    });
    const response = await fetch(
      `http://127.0.0.1:${s.address().port}/precise-wav/47`,
      { method: 'POST', body: new Uint8Array([1]) }
    );
    expect(response.status).toBe(501);
    await new Promise(resolve => s.close(resolve));
  });
});
