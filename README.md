<p align="center">
  <img src="images/logo.png" alt="Logo" width="156" height="156">
</p>

<h2 align="center" style="font-weight: 600">YesPlayMusic</h2>

<p align="center">高颜值的第三方网易云播放器 · 个人 macOS 定制版</p>

---

## 关于这个仓库

这是从 [qier222/YesPlayMusic](https://github.com/qier222/YesPlayMusic) 分出来的个人版本，
不再跟随上游发版，只按自己的使用习惯打磨 macOS 上的体验。原项目的功能和界面基本保留，
下面这些是这个版本额外做的事。

如果你是来找原版的，请直接去[上游仓库](https://github.com/qier222/YesPlayMusic)，
那边有完整的跨平台安装包和文档。

## 这个版本改了什么

**迷你播放器。** 把窗口拖窄（宽度小于 620 或高度小于 340）就会自动变成一条紧凑的播放条，
左边小封面配歌名歌手，中间是当前这句歌词，右边是播放控制。空间够的时候原文下面还会跟一行
中文翻译，纯音乐则显示「纯音乐，请欣赏」。拖回大窗口自动还原成完整歌词页。
原版最小窗口是 1080×720，这里放开到了 300×48。

**窗口置顶。** 迷你条上有个图钉按钮，点一下把窗口钉在最上层，切到别的桌面或者全屏应用上面
它也会跟着走，适合一边干活一边扫一眼歌词。按钮平时藏着，鼠标移到播放条上才浮现，
开关状态会记住，下次启动还在。迷你模式下红绿灯也默认收起，悬浮时才出现。

**macOS 菜单栏歌词。** 菜单栏图标位置直接显示专辑封面，右边跟着当前歌词走。
文字按显示宽度截断而不是字符数，所以中日文和英文都能显示得比较满。迷你条开着的时候
菜单栏只留封面，不重复显示一遍歌词；窗口一收起，歌词立刻补回菜单栏。

**Anon 进度条。** 在原有的彩虹猫之外多加了一种进度条皮肤，设置里可以切换，两者互斥。

**修了一个切歌的老问题。** 网络慢的时候快速切歌，前一首的歌词响应可能晚于后一首到达，
把新歌的歌词覆盖成上一首的。现在请求前会记住是哪首歌，回来发现已经切歌就直接丢弃。

**换掉了整套构建工具。** 原来的 Vue CLI 4（webpack 4）在新版 Node 上跑不起来，
开发模式也拉不起窗口。现在是 Vite 7 加 electron-vite 5，Electron 升到 43，
构建时间从三四分钟降到一秒半，改完代码存盘就生效。Vue 2 的业务代码没有动。

## 自己构建

需要 [bun](https://bun.sh)，Node 20 以上（Node 26 实测可用）。

```bash
bun install
bun run dev        # 开发，主进程和渲染进程都热重载
bun run build:app  # 出 macOS 安装包
```

产物在 `dist_electron/`，把 `mac-arm64/YesPlayMusic.app` 拷进 `/Applications` 就能用。
目前只配了 Apple Silicon 的 dmg，需要 Intel 或别的平台请改 `electron-builder.yml`。

开发细节和踩过的坑都记在 [CLAUDE.md](CLAUDE.md) 里。

## 致谢

这个项目的一切都建立在 [qier222](https://github.com/qier222) 和
[YesPlayMusic 所有贡献者](https://github.com/qier222/YesPlayMusic/graphs/contributors)
的工作之上。播放器内核、界面设计、歌词、音乐库、网易云 API 的对接，这些真正困难的部分
都是他们写好的，这个 fork 只是在上面加了几个自己想要的功能。

同样感谢这些被项目依赖的开源工作：

- [NeteaseCloudMusicApi](https://github.com/Binaryify/NeteaseCloudMusicApi) 及其
  [维护分支](https://github.com/neteasecloudmusicapienhanced/api)：网易云 API 的实现
- [UnblockNeteaseMusic](https://github.com/UnblockNeteaseMusic/server)：灰色歌曲解锁
- [Vue](https://vuejs.org)、[Vite](https://vite.dev)、[Electron](https://www.electronjs.org)

界面设计的灵感来自 [Apple Music](https://music.apple.com)、
[YouTube Music](https://music.youtube.com) 和 [Spotify](https://www.spotify.com)。

## 截图

![lyrics][lyrics-screenshot]
![library-dark][library-dark-screenshot]
![album][album-screenshot]
![home-2][home-2-screenshot]

## 开源许可

沿用上游的 [MIT license](https://opensource.org/licenses/MIT)。

仅供个人学习研究使用，禁止用于商业及非法用途。音乐版权归网易云音乐及各版权方所有。

<!-- MARKDOWN LINKS & IMAGES -->

[album-screenshot]: images/album.png
[home-2-screenshot]: images/home-2.png
[lyrics-screenshot]: images/lyrics.png
[library-dark-screenshot]: images/library-dark.png
