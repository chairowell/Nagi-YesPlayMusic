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
  test('位深映射保持无损：16→LEI16、24→LEI24、32→LEF32、异常回退 LEI16', () => {
    expect(afconvertDataFormat(16)).toBe('LEI16');
    expect(afconvertDataFormat(24)).toBe('LEI24');
    expect(afconvertDataFormat(32)).toBe('LEF32');
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
      convert: async (flacPath, wavPath, dataFormat) => {
        convertCalls.push({ flacPath, wavPath, dataFormat });
        if (dataFormat === 'LEF32') throw new Error('模拟转换失败');
        const flacBytes = await fsp.readFile(flacPath);
        // 假转换器：证明收到的就是 POST 的字节
        await fsp.writeFile(wavPath, Buffer.concat([Buffer.from('RIFF'), flacBytes]));
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

  test('非法 ID 与转换失败分别返回 400/500，失败不留垃圾文件', async () => {
    expect(
      (await fetch(`${base}/precise-wav/..%2Fevil`, { method: 'POST' })).status
    ).toBe(400);
    const failed = await fetch(`${base}/precise-wav/44?bits=32`, {
      method: 'POST',
      body: new Uint8Array([1]),
    });
    expect(failed.status).toBe(500);
    expect((await fsp.readdir(tempDir)).includes('44.flac')).toBe(false);
  });

  test('启动清扫会清空历史残留', async () => {
    writeFileSync(path.join(tempDir, 'stale.wav'), 'x');
    await sweepPreciseWavDir(tempDir);
    expect(await fsp.readdir(tempDir)).toEqual([]);
  });
});
