# 性能迁移基线

## 目标

Electron 与 Tauri 必须使用同一套口径比较，不能只看 Activity Monitor 里某一个 helper。
本项目按根 PID 递归统计整个进程树：

- 内存：各进程 RSS 相加，记录 mean / P95 / max；
- CPU：各进程 `%CPU` 相加，记录 mean / P95 / max；
- 体积：比较 `.app` 目录总大小；
- 场景：冷启动空闲、正常播放、窗口隐藏各采样 5 分钟。

RSS 会重复计算进程间共享页，因此不是物理内存的绝对值；但两版使用相同采样器，适合做相对比较。

## 迁移前静态基线（2026-08-02）

| 项目 | Electron 版 | 说明 |
| --- | ---: | --- |
| `.app` 总大小 | 381.5 MiB | `dist_electron/mac-arm64/YesPlayMusic.app` |
| renderer 全部资源 | 4.13 MiB | `out/renderer` 所有文件 |
| Bun arm64 sidecar | 63.9 MiB | 单文件，包含 Bun runtime 与 1,077 个模块 |

此前的探索性采样中，Electron 整棵进程树 RSS 约为 383–722 MiB，Electron Framework
自身约 273 MiB。这个范围只用于判断优化量级；正式验收必须用下面的固定场景重新采样。

## Tauri 后台核心中间结果（2026-08-02）

`bun run smoke:tauri` 在不创建 WebView、不显示窗口的条件下，验证 production bundle
里的真实 sidecar、静态页面、同源 API 和退出回收：

| 项目 | 结果 |
| --- | ---: |
| `.app` 总大小 | 73.4 MiB |
| Tauri 主进程 RSS | 79.2 MiB |
| Bun sidecar RSS | 90.25 MiB |
| 两进程 RSS mean / P95 | 169.54 / 169.56 MiB |
| 两进程 CPU mean / P95 | 0.38% / 1.3% |

包体积相对 Electron 下降约 80.8%。后台核心 RSS 相比此前 Electron 探索性范围低约
55.7%–76.5%，但这个数字**不含 WKWebView 的 WebContent / Networking 进程**，只能说明
Rust + Bun 后台的固定成本，不能当作最终播放器内存。完整结果要等隐藏 WebView 和正常播放
场景接入后再测。

## 采样方法

先拿到被测版本的**精确根 PID**，再运行：

```bash
bun scripts/measure-process-tree.mjs \
  --pid 12345 \
  --duration 300 \
  --interval 1 \
  --label electron-hidden
```

工具只读取指定 PID 及其后代，不按应用名扫描，也不会启动、聚焦或关闭播放器。

## 正常播放场景实测（2026-08-10，Apple Silicon / macOS 15）

首次按完整口径（含 WKWebView 各进程）测量正常播放场景，并发现、修复了一个
CPU 回归：

- **修复前**：Tauri 主进程恒定约 99% CPU（`MainEventsCleared` 每轮迭代刷新托盘
  标题，查询窗口状态唤醒 run loop 形成自持续空转），与负载无关；
- **修复后**（托盘标题改事件驱动 + 1s 对账线程，commit 64b680c）：播放态主进程
  6–8%，空闲更低；60 秒进程树采样 CPU mean 8.5% / P95 16.7%。

内存（`footprint` 的 phys_footprint 口径，非 RSS）：

| 进程 | phys_footprint |
| --- | ---: |
| Tauri 主进程 | 58 MB |
| Bun sidecar | 82 MB |
| WebKit WebContent | 444 MB（观测峰值 1.76 GB） |
| 合计 | 约 605 MB |

高强度交互（连续切歌、搜索、调窗口）时 WebContent 峰值约 49% CPU、GPU 进程约
67%，操作结束后均回落。RSS 含大量共享页，不作为对外数字；对外只引用
phys_footprint 与场景说明。用户缓存（`~/Library/WebKit/com.electron.yesplaymusic`
等）随使用增长，与安装体积分开陈述。

## Tauri 验收线

- 隐藏窗口 CPU mean 不高于 2%，且明显低于修复前约 30% 的探索性结果；
- 播放态主进程 CPU mean 不高于 10%（2026-08-10 实测 6–8%，回归门禁）；
- 空闲 RSS mean 相比 Electron 至少下降 40%；
- 正常播放 10 分钟内 RSS 不持续单调上涨；
- 随机播放、缓存、登录 cookie、托盘歌词不因降内存而回归。
