# YesPlayMusic（个人 fork）

高颜值的第三方网易云播放器，本仓库是从 [qier222/YesPlayMusic](https://github.com/qier222/YesPlayMusic)
分出来独立维护的私有版本，只针对 macOS 做打磨。

## 命令

| 用途 | 命令 |
| --- | --- |
| 开发（主进程 + 渲染进程热重载） | `bun run dev` |
| 出 macOS 安装包 | `bun run build:app` |
| 只构建渲染进程（浏览器里调 UI） | `bun run build:renderer` |

产物在 `dist_electron/mac-arm64/YesPlayMusic.app`，拷进 `/Applications` 覆盖即可。

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

Vue 3.5 + Vuex 4 + Vue Router 4 + Vite 7 + electron-vite 5 + Electron 43，包管理用 bun。
业务代码保留选项式 API，没有 TypeScript。

原项目用的是 Vue CLI 4（webpack 4），在 node 26 上跑不起来、`electron:serve` 会卡死。
构建链已整体迁到 Vite，构建时间从三四分钟降到约 1.5 秒。渲染进程和主进程的配置都在
`electron.vite.config.mjs`；`vite.config.mjs` 是纯 Web 模式（浏览器里开发 UI）用的。

## 架构要点

主进程入口 `src/background.js`。它做三件事：在 10754 端口起网易云 API 服务、
在 27232 端口起一个 Express 服务托管渲染进程产物并把 `/api` 转发到 10754、创建窗口。

**生产模式不走 `app://` 协议**，而是 `loadURL('http://localhost:27232')`。
dev 模式则加载 `ELECTRON_RENDERER_URL`。dev 的 Vite server 也配了 `/api` 同源代理
指向 10754 —— 这个不能省，否则 5173 到 10754 属于跨站，登录 cookie 会被
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

1. `bun install` 默认拦截依赖的 postinstall。`postinstall` 已改成
   `npm_execpath= electron-builder install-app-deps`：bun 把 `npm_execpath` 指向自己，
   清空它 electron-builder 才会走 npm。修之前每次装包都报错，而且**失败时 bun 不会把
   新依赖写进 package.json**，看起来像装了其实没装。
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
