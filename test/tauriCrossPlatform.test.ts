import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import {
  hostTargetTriple,
  sidecarBuildPlan,
} from '../scripts/build-sidecar.mjs';
import { tauriHostBuildPlan } from '../scripts/build-tauri-host.mjs';

const packageJson = JSON.parse(readFileSync('package.json', 'utf8'));
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
});
