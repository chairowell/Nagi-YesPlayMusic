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

## Tauri 验收线

- 隐藏窗口 CPU mean 不高于 2%，且明显低于修复前约 30% 的探索性结果；
- 空闲 RSS mean 相比 Electron 至少下降 40%；
- 正常播放 10 分钟内 RSS 不持续单调上涨；
- 随机播放、缓存、登录 cookie、托盘歌词不因降内存而回归。
