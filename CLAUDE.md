# YesPlayMusic（个人 fork）

高颜值的第三方网易云播放器，本仓库是从 [qier222/YesPlayMusic](https://github.com/qier222/YesPlayMusic)
分出来独立维护的私有版本。Apple Silicon macOS 是正式支持平台；Windows x64 和
Ubuntu x64 由 CI 提供 Tauri 实验构建。

## 命令

| 用途                               | 命令                          |
| ---------------------------------- | ----------------------------- |
| Tauri 开发                         | `bun run dev:tauri`           |
| 按当前系统出 Tauri 安装包          | `bun run build:tauri`         |
| Windows x64 NSIS 安装包            | `bun run build:tauri:windows` |
| Ubuntu x64 AppImage + deb          | `bun run build:tauri:linux`   |
| 旧 Electron 开发（只用于回归对照） | `bun run dev`                 |
| 只构建渲染进程（浏览器里调 UI）    | `bun run build:renderer`      |

Tauri 产物在 `src-tauri/target/<target-triple>/release/bundle/`。macOS 正式发布仍通过
`bun run package:tauri:dmg` 收集到 `dist_tauri/`。

`bun run dev` 背后是 `electron-vite dev --watch`。**`--watch` 不能省** —— 没有它，
改主进程代码不会重建，窗口也不会重启，很容易误判成"代码没生效"。

## 提交前的验证

`.githooks/pre-commit` 会跑 `bun test`（0.5 秒）和 `bun run build:tauri:renderer`（1.5 秒）。
`bun install` 时的 `prepare` 会把 `core.hooksPath` 指过去，新 clone 也自动生效。

两步缺一不可：测试不 import `.vue`，所以"import 了一个不存在的模块"只有渲染构建能发现——
2026-08-04 就是这么把临时探针的残留 import 提交进去的，HEAD 里 import 了一个仓库里
根本不存在的文件。

CI（`.github/workflows/build.yaml`）只验证每次 push 的**最后一个 commit**，一次推 21 个
中间那 20 个不会被碰，所以这道关必须在本地。

## 发版

版本号要同时改三处：`package.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`
（Cargo.lock 跟着更新）。`bun run verify:tauri:version` 会校验三者与 tag 一致，CI 里也会跑。

推 `v*` tag 触发 `.github/workflows/build.yaml`：公证构建 → 建**草稿** release。
草稿要手动发布：`gh release edit vX.Y.Z --draft=false --latest`。

**发布前必须手写 release 正文**，不能只留自动生成的 Full Changelog 链接。
仓库没有 CHANGELOG 文件，变更记录只存在于 release 正文里。格式照 v0.6.2 / v0.6.3：
一段 `## 修复`，用户视角的中文条目（说"能拖动窗口了"，不说"补了 drag-region 属性"），
末尾保留自动追加的 Full Changelog 那一行、不要自己再写一遍（v0.6.2 就重了）。

## 技术栈

Vue 3.5 + Vuex 4 + Vue Router 4 + Vite 7 + Tauri 2；旧 Electron 43 只保留回归对照，包管理用 bun。
业务代码保留选项式 API，没有 TypeScript。

原项目用的是 Vue CLI 4（webpack 4），在 node 26 上跑不起来、`electron:serve` 会卡死。
构建链已整体迁到 Vite，构建时间从三四分钟降到约 1.5 秒。渲染进程和主进程的配置都在
`electron.vite.config.mjs`；`vite.config.mjs` 是纯 Web 模式（浏览器里开发 UI）用的。

## 架构要点

Tauri 主进程入口是 `src-tauri/src/main.rs`，负责窗口、托盘、快捷键、单实例和 Sidecar
生命周期。`src/sidecar.js` 会编译成各平台独立可执行文件，负责网易云 API、托管渲染
产物、同源 `/api` 代理和 UNM。正式版页面来自 `http://127.0.0.1:28232`。

`src/background.js` 是旧 Electron 主进程，只用于迁移回归对照，不再作为 Windows/Linux
测试包发布。

**生产模式不走 `app://` 协议**，而是加载 Sidecar 的 loopback HTTP 页面。
dev 的 Vite server 也配了 `/api` 同源代理指向 12754 —— 这个不能省，否则跨端口属于跨站，登录 cookie 会被
Chromium 的 SameSite 策略丢掉，表现为头像不刷新、library 空。

迷你播放器做在 `src/views/lyrics.vue` 里：窗口宽 < 620 或高 < 340 自动切成紧凑播放条，
`src/App.vue` 负责在窗口变窄时自动切到歌词页。macOS 菜单栏的封面和歌词在
`src/electron/tray.js` 的 `YPMTrayMacImpl`。

## 数据目录（容易搞错）

`~/Library/Application Support/yesplaymusic` 是同一个目录，但 Chromium 在里面按 origin
再分一层，而 dev 和正式版端口不同：

- **共用**：cookie（只认域名不认端口，dev 登录了正式版也是登录的）、electron-store
  的 JSON（窗口尺寸、置顶状态）
- **不共用**：IndexedDB 歌曲缓存、localStorage 设置。dev 在 `http_localhost_5173`，
  正式版在 `http_localhost_27232`，各存各的

`Local Storage/leveldb` 是所有 origin 共用的**一个文件**，删它会把正式版设置一起清掉。
清理 dev 缓存只删 `IndexedDB/http_localhost_5173.*`。

## 已知的坑

1. `bun install` 默认拦截依赖的 postinstall。`postinstall` 通过跨平台 Node 脚本清空
   `npm_execpath` 后再调用 electron-builder；不能改回 POSIX 的 `npm_execpath= ...` 前缀，
   Windows shell 不保证支持。安装失败时 bun 不会把新依赖写进 package.json，看起来像装了其实没装。
2. `electron` 和 `electron-builder` 必须待在 `devDependencies`，否则 electron-builder
   直接拒绝打包。
3. `src/ncmModDef.js` 是 CommonJS 写法，必须静态 `import` 并在
   `electron.vite.config.mjs` 的 `commonjsOptions.include` 里点名 —— rollup 把 `src`
   下的 `.js` 一律当 ESM 解析，漏了它网易云 API 起不来。
4. `vite-plugin-svg-icons` 只在 dev server 启动时扫一遍 `src/assets/icons`，
   新加的 svg 要重启 dev 才会进 sprite，否则图标位置是空白。
5. `.player` 上有 `backdrop-filter`，超出它上边界的子元素会被裁掉 —— 进度条上的角色
   容易缺一块头。
6. 单实例锁：`/Applications` 里的正式版开着时，新起的实例会静默 `app.quit()`，
   看起来像打包失败。测试前先退掉。
7. 卸载 brew cask 时**不要加 `--zap`**，会连数据目录一起删。

## 约定

- 中文注释，解释"为什么"而不是"做了什么"
- 提交信息用中文，正文说清动机和影响
- 上游仓库是 `upstream` remote，同步用 `git fetch upstream`
