import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { mkdtemp, readFile, rm, stat, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import {
  hostTargetTriple,
  sidecarBuildPlan,
  writeLinuxSidecarBundle,
} from '../scripts/build-sidecar.mjs';
import { tauriHostBuildPlan } from '../scripts/build-tauri-host.mjs';

const packageJson = JSON.parse(readFileSync('package.json', 'utf8'));
const tauriConfig = JSON.parse(
  readFileSync('src-tauri/tauri.conf.json', 'utf8')
);
const windowsConfig = JSON.parse(
  readFileSync('src-tauri/tauri.windows.conf.json', 'utf8')
);
const linuxConfig = JSON.parse(
  readFileSync('src-tauri/tauri.linux.conf.json', 'utf8')
);
const rustMain = readFileSync('src-tauri/src/main.rs', 'utf8');
const linuxMedia = readFileSync('src-tauri/src/linux_media.rs', 'utf8');

describe('Tauri 跨平台 Sidecar', () => {
  test('三个受支持平台生成 Tauri 要求的 target triple 文件名', () => {
    const mac = sidecarBuildPlan({ targetTriple: 'aarch64-apple-darwin' });
    const windows = sidecarBuildPlan({
      targetTriple: 'x86_64-pc-windows-msvc',
    });
    const linux = sidecarBuildPlan({
      targetTriple: 'x86_64-unknown-linux-gnu',
    });

    expect(mac.outputName).toBe('yesplaymusic-sidecar-aarch64-apple-darwin');
    expect(windows.outputName).toBe(
      'yesplaymusic-sidecar-x86_64-pc-windows-msvc.exe'
    );
    expect(linux.outputName).toBe(
      'yesplaymusic-sidecar-x86_64-unknown-linux-gnu'
    );
    expect(windows.args).toContain('--target=bun-windows-x64-baseline');
    expect(windows.args).toContain('--windows-hide-console');
    expect(linux.args).toContain('--target=bun-linux-x64-baseline');
    expect(linux.usesPayloadWrapper).toBe(true);
    expect(linux.compileOutputPath).toEndWith('.raw');
    expect(linux.payloadPath).toEndWith('yesplaymusic-sidecar-linux.payload');
  });

  test('Linux Sidecar 以不被 linuxdeploy 改写的 payload 原样封装', async () => {
    const root = await mkdtemp(path.join(tmpdir(), 'ypm-linux-sidecar-'));
    const compileOutputPath = path.join(root, 'sidecar.raw');
    const outputPath = path.join(root, 'yesplaymusic-sidecar');
    const payloadPath = path.join(root, 'yesplaymusic-sidecar-linux.payload');
    const original = new Uint8Array([0x7f, 0x45, 0x4c, 0x46, 0, 1, 2, 3]);
    try {
      await writeFile(compileOutputPath, original);
      const { digest } = writeLinuxSidecarBundle({
        compileOutputPath,
        outputPath,
        payloadPath,
      });
      const payload = await readFile(payloadPath);
      const wrapper = await readFile(outputPath, 'utf8');

      expect(payload.subarray(0, 4).toString()).toBe('YPM1');
      expect(Array.from(payload.subarray(4))).toEqual(Array.from(original));
      expect(wrapper).toContain(`sidecar-${digest}`);
      expect(wrapper).toContain('exec "$cached" "$@"');
      if (process.platform !== 'win32') {
        expect((await stat(outputPath)).mode & 0o111).not.toBe(0);
        expect((await stat(payloadPath)).mode & 0o444).toBe(0o444);
      }
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });

  test('本机平台映射到对应 Rust target，不再写死 macOS', () => {
    expect(hostTargetTriple({ platform: 'darwin', arch: 'arm64' })).toBe(
      'aarch64-apple-darwin'
    );
    expect(hostTargetTriple({ platform: 'win32', arch: 'x64' })).toBe(
      'x86_64-pc-windows-msvc'
    );
    expect(hostTargetTriple({ platform: 'linux', arch: 'x64' })).toBe(
      'x86_64-unknown-linux-gnu'
    );
  });
});

describe('Tauri 本机安装包', () => {
  test('统一 build 命令按宿主选择 macOS、Windows 或 Linux 构建', () => {
    expect(
      tauriHostBuildPlan({ platform: 'darwin', arch: 'arm64' }).script
    ).toBe('build:tauri:macos');
    expect(tauriHostBuildPlan({ platform: 'win32', arch: 'x64' }).script).toBe(
      'build:tauri:windows'
    );
    expect(tauriHostBuildPlan({ platform: 'linux', arch: 'x64' }).script).toBe(
      'build:tauri:linux'
    );
  });

  test('Windows 输出 NSIS exe，Ubuntu 输出 AppImage 和 deb', () => {
    expect(windowsConfig.bundle.targets).toEqual(['nsis']);
    expect(windowsConfig.bundle.windows.nsis.installMode).toBe('currentUser');
    expect(linuxConfig.bundle.targets).toEqual(['appimage', 'deb']);
    expect(linuxConfig.bundle.linux.appimage.files).toEqual({
      '/usr/lib/yesplaymusic/sidecar.payload':
        'binaries/yesplaymusic-sidecar-linux.payload',
    });
    expect(linuxConfig.bundle.linux.deb.files).toEqual(
      linuxConfig.bundle.linux.appimage.files
    );
    // Tray icons dlopen the appindicator library; apt must install it with the deb.
    expect(linuxConfig.bundle.linux.deb.depends).toEqual([
      'libayatana-appindicator3-1',
    ]);
    expect(packageJson.scripts['build:tauri:windows']).toContain(
      'x86_64-pc-windows-msvc'
    );
    expect(packageJson.scripts['build:tauri:linux']).toContain(
      'x86_64-unknown-linux-gnu'
    );
  });

  test('Rust 外壳不再依赖 Windows 不存在的 /dev/urandom', () => {
    expect(rustMain).toContain('getrandom::getrandom(&mut bytes)');
    expect(rustMain).not.toContain('File::open("/dev/urandom")');
  });

  test('macOS 独有的 Reopen 事件不进入 Windows/Linux 编译', () => {
    expect(rustMain).toContain(
      '#[cfg(target_os = "macos")]\n        RunEvent::Reopen'
    );
  });

  test('Windows release 主程序和 Sidecar 都不弹命令行窗口', () => {
    expect(rustMain).toContain('windows_subsystem = "windows"');
    const windows = sidecarBuildPlan({
      targetTriple: 'x86_64-pc-windows-msvc',
    });
    expect(windows.args).toContain('--windows-hide-console');
  });

  test('OSDLyrics 投递不占用 MPRIS 服务循环', () => {
    expect(linuxMedia).toContain('yesplaymusic-osdlyrics');
    expect(linuxMedia).toContain('MediaUpdate::LyricsDelivered');
    expect(linuxMedia).not.toContain(
      'deliver_osd_lyrics(&lyrics, osd_started).await'
    );
  });

  test('MPRIS 初始化不会阻塞 Sidecar 和主窗口启动', () => {
    expect(linuxMedia).toContain(
      'futures_lite::future::block_on(run_media_service(thread_queue, control_handler))'
    );
    expect(linuxMedia).not.toContain('startup_rx.recv');
  });

  test('MPRIS DesktopEntry 与 Linux 安装包的 desktop 文件同名', () => {
    expect(linuxMedia).toContain(
      `.desktop_entry("${tauriConfig.productName}")`
    );
  });

  test('macOS 托盘封面只在主线程更新 AppKit 状态', () => {
    const start = rustMain.indexOf('fn update_tray_cover(');
    const end = rustMain.indexOf('\nfn update_tray_menu(', start);
    const updateTrayCover = rustMain.slice(start, end);

    expect(start).toBeGreaterThan(-1);
    expect(end).toBeGreaterThan(start);
    expect(updateTrayCover).toContain('run_on_main_thread');
    expect(updateTrayCover).toMatch(
      /run_on_main_thread\([\s\S]*tray_by_id\("main-tray"\)[\s\S]*tray\.set_icon/
    );
  });

  test('托盘不可用时仍继续创建主窗口', () => {
    expect(rustMain).toContain('.plugin(tauri_plugin_dialog::init())');
    expect(rustMain).toContain('if let Err(error) = create_tray(app)');
    expect(rustMain).not.toContain('create_tray(app)?;');
  });

  test('托盘标题走事件驱动加慢速对账，不挂在事件循环轮询上', () => {
    // MainEventsCleared 轮询会自唤醒 run loop，曾造成主进程恒定 99% CPU。
    expect(rustMain).not.toContain('RunEvent::MainEventsCleared');
    expect(rustMain).toContain('spawn_tray_title_reconciler');
    expect(rustMain).toMatch(
      /spawn_tray_title_reconciler[\s\S]*?thread::sleep\(TRAY_TITLE_RECONCILE_INTERVAL\)[\s\S]*?run_on_main_thread/
    );
  });
});
