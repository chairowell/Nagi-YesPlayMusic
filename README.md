<p align="center">
  <img src="images/logo.png" alt="YesPlayMusic Logo" width="156" height="156">
  &nbsp;&nbsp;&nbsp;&nbsp;
  <a href="https://tauri.app"><img src="images/tauri-glyph.svg" alt="Built with Tauri 2" height="72"></a>
</p>

<h2 align="center" style="font-weight: 600">YesPlayMusic</h2>

<p align="center">高颜值的第三方网易云播放器</p>
<p align="center"><sub>Tauri 2 重构版 · macOS 正式发布 · Windows / Ubuntu 实验构建 · 由 <a href="https://github.com/nagi-studio">Nagi Studio</a> 维护</sub></p>

---

## 关于这个仓库

这是从 [qier222/YesPlayMusic](https://github.com/qier222/YesPlayMusic) 分出来独立维护的
macOS Tauri 重构版，不再跟随上游发版。原项目的界面和主要功能保留下来，桌面运行时、
本地服务、缓存和窗口交互已经针对 macOS 重新实现。

如果你是来找原版的，请直接去[上游仓库](https://github.com/qier222/YesPlayMusic)，
那边有完整的跨平台安装包和文档。

**欢迎提 Issue 和 PR。** Apple Silicon Mac 是正式支持平台；Windows x64 和 Ubuntu x64
由 CI 提供实验构建，等待更多真实设备反馈。

## Tauri 重构版改了什么

**v0.6.0 是一次完整的 Tauri 重构。** 渲染层升级到 Vue 3 和 Vite 7，桌面外壳改为
Tauri 2。应用不再捆绑 Chromium，窗口、菜单栏、媒体状态和本地服务由 Rust 主进程接管。

包体积是目前可以直接复现的收益：迁移前 Electron 版 `.app` 为 381.5 MiB，当前
Tauri v0.6.0 约为 80.8 MiB，减少约 79%。内存仍按完整进程树继续测量，已有的后台核心
数据没有包含 WKWebView，因此这里不写不完整的内存降幅。测试口径和阶段性结果见
[性能迁移基线](docs/performance-baseline.md)。

**迷你播放器。** 把窗口拖窄（宽度小于 620 或高度小于 340）就会自动变成一条紧凑的播放条，
左边小封面配歌名歌手，中间是当前这句歌词，右边是播放控制。空间够的时候原文下面还会跟一行
中文翻译，纯音乐则显示「纯音乐，请欣赏」。拖回大窗口自动还原成完整歌词页。
原版最小窗口是 1080×720，这里放开到了 300×48。

**纯音乐**

![迷你播放器：纯音乐](images/mini-player-instrumental.png)

**双语歌词**

![迷你播放器：双语歌词（Venice Bitch）](images/mini-player-bilingual.png)

![迷你播放器：双语歌词（Red）](images/mini-player.png)

**窗口置顶。** 迷你条上有个图钉按钮，点一下把窗口钉在最上层，切到别的桌面或者全屏应用上面
它也会跟着走，适合一边干活一边扫一眼歌词。按钮平时藏着，鼠标移到播放条上才浮现，
开关状态会记住，下次启动还在。迷你模式下红绿灯也默认收起，悬浮时才出现。

**macOS 菜单栏歌词。** 菜单栏图标位置直接显示专辑封面，右边跟着当前歌词走。
文字按显示宽度截断而不是字符数，所以中日文和英文都能显示得比较满。迷你条开着的时候
菜单栏只留封面，不重复显示一遍歌词；窗口一收起，歌词立刻补回菜单栏。

![菜单栏歌词](images/menubar.png)

**Anon 进度条。** 在原有的彩虹猫之外多加了一种进度条皮肤，设置里可以切换，两者互斥。

**修了一个切歌的老问题。** 网络慢的时候快速切歌，前一首的歌词响应可能晚于后一首到达，
把新歌的歌词覆盖成上一首的。现在请求前会记住是哪首歌，回来发现已经切歌就直接丢弃。

## 安装

到 [Releases](https://github.com/nagi-studio/YesPlayMusic/releases) 下载 dmg。
当前版本只提供 Apple Silicon（`arm64`）安装包。

安装包没有 Developer ID 签名和 Apple 公证，首次打开时 macOS 会拦一道。放行方法二选一：

- 打开「系统设置 → 隐私与安全性」，往下翻到被拦截的提示，点「仍要打开」
- 或者在终端跑一句：`xattr -dr com.apple.quarantine /Applications/YesPlayMusic.app`

自己从源码构建的话没有这个问题，本地产物不带隔离属性。

## 自己构建

需要 [bun](https://bun.sh)，Node 20 以上（Node 26 实测可用）。

```bash
cp .env.example .env   # 必须，缺了它前端拿不到 API 地址，界面会全空
bun install
bun run dev:tauri      # 开发模式
bun run build:tauri    # 按当前系统构建 Tauri 应用
bun run package:tauri:dmg  # 生成可分发的 DMG 和 SHA-256 校验文件
```

`.env` 不进版本库，但 `.env.example` 里已经是一份可以直接用的完整配置，
不需要自己去申请任何密钥。

应用产物在
`src-tauri/target/aarch64-apple-darwin/release/bundle/macos/YesPlayMusic.app`，
DMG 和校验文件在 `dist_tauri/`。

版本 tag 默认也走这套无 Developer ID 的发布流程。以后需要正式签名和公证时，
在仓库中设置 `APPLE_SIGNING_ENABLED=true` 并补齐 Apple 凭据即可；对应 CI 步骤仍然保留。

开发细节和踩过的坑都记在 [CLAUDE.md](CLAUDE.md) 里。

## 关于 Windows 和 Linux

Windows x64 和 Ubuntu x64 使用与 macOS 相同的 Tauri 外壳，不再发布旧 Electron 外壳。
每次向仓库自己的分支 push 后，可以在 Actions 的 `Desktop builds` 运行记录底部下载：

- Windows：未签名的 NSIS `.exe` 安装包
- Ubuntu：AppImage 和 `.deb`

这些实验包暂时不自动加入 GitHub Release。Windows 未签名安装包可能触发 SmartScreen；
不要为此全局关闭杀毒软件或 SmartScreen。AppImage 可以直接运行，`.deb` 适合 Ubuntu / Debian。

在对应系统本机从源码构建：

```bash
bun run build:tauri:windows  # Windows x64，输出 NSIS setup.exe
bun run build:tauri:linux    # Ubuntu x64，输出 AppImage 和 deb
```

Tauri 会同时编译目标平台的 Bun Sidecar。Sidecar 是随应用安装的本地后端，只监听
`127.0.0.1`，负责网易云 API、同源登录代理和 UNM；用户电脑不需要另装 Bun。
Windows / Linux 没有 macOS 的 `afconvert`，精确 FLAC 拖动会自动使用播放器已有的回退路径。

## 致谢

这个项目的一切都建立在 [qier222](https://github.com/qier222) 和
[YesPlayMusic 所有贡献者](https://github.com/qier222/YesPlayMusic/graphs/contributors)
的工作之上。播放器内核、界面设计、歌词、音乐库、网易云 API 的对接，这些真正困难的部分
都是他们写好的，这个分支只是在上面加了几个自己想要的功能。

同样感谢这些被项目依赖的开源工作：

- [NeteaseCloudMusicApi](https://github.com/Binaryify/NeteaseCloudMusicApi) 及其
  [维护分支](https://github.com/neteasecloudmusicapienhanced/api)：网易云 API 的实现
- [UnblockNeteaseMusic](https://github.com/UnblockNeteaseMusic/server)：灰色歌曲解锁
- [Vue](https://vuejs.org)、[Vite](https://vite.dev)、[Tauri](https://tauri.app)

界面设计的灵感来自 [Apple Music](https://music.apple.com)、
[YouTube Music](https://music.youtube.com) 和 [Spotify](https://www.spotify.com)。

## 截图

以下界面来自上游，这个分支没有改动。

![歌词页][lyrics-screenshot]

![音乐库（深色）][library-dark-screenshot]

![专辑][album-screenshot]

## 开源许可

沿用上游的 [MIT license](https://opensource.org/licenses/MIT)。

仅供个人学习研究使用，禁止用于商业及非法用途。音乐版权归网易云音乐及各版权方所有。

TAURI is a trademark of The Tauri Programme within the Commons Conservancy. README 中使用的
Tauri 标识来自官方 Logopack，仅作技术栈说明。

<!-- MARKDOWN LINKS & IMAGES -->

[album-screenshot]: images/album.png
[lyrics-screenshot]: images/lyrics.png
[library-dark-screenshot]: images/library-dark.png
