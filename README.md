<div align="center">

# DevScope

**macOS 开发者工具箱 · 本地开发环境诊断与项目分析**

原生桌面应用 · Tauri 2 · Rust · Vue 3 + TypeScript

[![Rust](https://img.shields.io/badge/rust-1.100.0-000000?logo=rust&logoColor=white)](#)
[![Tauri](https://img.shields.io/badge/tauri-2.12.7-24C8D8?logo=tauri&logoColor=white)](#)
[![Vue](https://img.shields.io/badge/vue-3.5.46-4FC08D?logo=vuedotjs&logoColor=white)](#)
[![Vite](https://img.shields.io/badge/vite-8.3.8-646CFF?logo=vite&logoColor=white)](#)
[![TypeScript](https://img.shields.io/badge/typescript-6.0.3-3178C6?logo=typescript&logoColor=white)](#)
[![macOS](https://img.shields.io/badge/macos-12%2B-lightgrey?logo=apple&logoColor=white)](#)
[![License](https://img.shields.io/badge/license-MIT-green)](LICENSE)

🚧 **早期版本** —— 核心功能已可用；代码签名、公证与 CI 仍在打磨。

</div>

---

## 项目简介

DevScope 是一个面向移动端开发者的 macOS 开发环境诊断工具，使用 **Tauri 2 + Vue 3 + TypeScript + Rust** 开发。

它把「本机到底装没装好开发环境」从终端里的一串命令，变成一份可读、可导出、可对比的界面：Flutter / Android / iOS / 常用工具链各自检测一遍，缺什么、装在哪、版本多少一目了然，再按需要汇总成 JSON 或 Markdown 诊断报告。

**以 Flutter 环境检测为例，一次完整的调用链路是这样的：**

1. 用户在 Vue 页面点击检测，前端通过统一封装的 Tauri IPC 接口调用 Rust command，并传入「是否强制刷新」的参数。
2. Rust command 层只做三件事：接收请求、把阻塞任务调度到 blocking 线程池、把机制性失败转换成前端可读的错误。
3. 真正的检测逻辑在 service 层：定位并执行本机 Flutter 工具链命令、解析输出、把结果组织成结构化模型。检测要启动外部进程，所以走 blocking 线程池，不占用 Tauri 的异步运行时。
4. 结果经 serde 序列化后通过 IPC 返回，前端按类型化的数据展示环境状态。
5. 重复检测由后端 30 秒 TTL 缓存兜底；用户手动刷新时传 `force = true` 直接绕过缓存。

分层边界是硬性要求：系统级操作（执行命令、读文件、探测环境变量）只写在 Rust，Vue 只负责展示和交互，两层之间只有 IPC。

## 检测链路

![DevScope Flutter 环境检测全链路：Vue 前端 → Tauri IPC → Rust commands → services → 结果展示](docs/images/flutter-detection-flow.png)

<sub>图中的代码片段取自实际实现：前端 `detectFlutterEnvironment()` → `call("detect_flutter_environment", { force })` → `commands::environment::detect_flutter_environment` → `services::flutter::detect()`。</sub>

| 环节 | 代码位置 | 职责 |
|---|---|---|
| 页面状态 | `src/composables/useDetection.ts` | loading / IPC 错误 / 结果状态机，四个检测页共用 |
| 统一 IPC | `src/services/rust-api.ts` | 所有 `invoke()` 集中一处，错误统一收敛成可读文案 |
| 命令层 | `src-tauri/src/commands/` | 参数进出 + `run_blocking` 调度，不含业务逻辑 |
| 服务层 | `src-tauri/src/services/` | 可执行文件发现、执行命令、解析版本、判定问题 |
| 缓存 | `src-tauri/src/services/cache.rs` | 30 秒 TTL，`force` 可绕过 |
| 数据契约 | `src-tauri/src/models/` ↔ `src/types/` | Rust struct / enum 与 TS interface 一一对应 |

## 功能特性

| 场景 | 说明 | 当前状态 |
|------|------|----------|
| 总览仪表盘 | 本机配置 + 常用开发环境与语言入口 | 已完成 |
| 本机配置 | macOS / 芯片 / 内存 / CPU 核心 / 机型 | 已完成 |
| Flutter 环境检测 | 定位 SDK、解析版本，按需运行 `flutter doctor -v` | 已完成 |
| Android 环境检测 | SDK / platform / build-tools / JDK / licenses / AVD | 已完成 |
| iOS 环境检测 | Xcode / CLT / CocoaPods / Swift / iOS 模拟器 | 已完成 |
| 常用工具链检测 | Python / Node.js / Git / Rust / Go / Java 的路径与版本 | 已完成 |
| Flutter 项目分析 | pubspec / 依赖筛选 / 资源 / Gradle / iOS 配置 / 依赖更新检查 | 已完成 |
| 构建日志解析 | 拖放读取、错误聚合、复制与导出筛选结果 | 已完成 |
| 诊断报告导出 | 汇总环境 / 项目 / 日志，JSON / Markdown、历史对比 | 已完成 |
| 检测缓存 | 30 秒内复用环境与工具链结果，手动刷新可强制绕过 | 已完成 |
| 界面体验 | 深色模式、Cmd+K、路径操作、主次按钮、页面过渡 | 已完成 |
| 关于 / 许可 | 版本、MIT License、项目链接与数据策略 | 已完成 |
| 菜单栏常驻 | 状态栏图标，可显示主窗口 / 关于 / 退出 | 已完成 |
| macOS 打包 | 输出 `.app` / `.dmg`（ad-hoc 签名，未公证） | 已完成 |

## 技术栈

| 层 | 技术 |
|---|---|
| 桌面框架 | Tauri 2 |
| 后端 | Rust (edition 2024) |
| 前端 | Vue 3 + TypeScript + Vue Router |
| 构建 | Vite 8 |
| 目标平台 | macOS (Apple Silicon) |

## 环境要求

- macOS 12+
- Rust 1.85+（edition 2024 的最低要求，[rustup](https://rustup.rs)）
- Node.js 20+
- Xcode Command Line Tools（`xcode-select --install`）

## 快速开始

```bash
npm install          # 安装前端依赖
npm run tauri dev    # 启动开发模式（会自动编译 Rust 并打开窗口）
```

打包发布版本：

```bash
npm run tauri build  # 产物在 src-tauri/target/release/bundle/
```

## 项目结构

```
devscope/
├── docs/
│   ├── PROJECT-STRUCTURE.md     逐文件说明
│   └── images/                  文档与 README 引用的架构图
├── src/                         前端（Vue 3 + TypeScript）
│   ├── router/                  路由（hash history）
│   ├── views/                   总览 / 环境检测 / 项目 / 日志 / 报告
│   ├── components/              可复用展示组件
│   ├── composables/             跨页面复用的组合式逻辑（useDetection）
│   ├── services/                rust-api、原生对话框、错误 / 状态 / 格式化
│   ├── types/                   index.ts：Rust 契约；ui.ts：展示型类型
│   └── styles/app.css           设计令牌与共享样式
└── src-tauri/                   Rust 后端
    └── src/
        ├── tray.rs              菜单栏常驻图标与菜单
        ├── commands/            前端可调用的命令（薄转发层）
        ├── services/            环境检测 + 项目 / 日志 / 报告分析
        ├── models/              前后端数据契约
        └── utils/               进程执行、文件读取、时间与版本处理
```

详细的逐文件说明见 [docs/PROJECT-STRUCTURE.md](docs/PROJECT-STRUCTURE.md)。

## 开发路线

- [x] 工程初始化（Tauri 2 + Vue 3 + TypeScript）
- [x] 本机配置与总览仪表盘
- [x] Flutter 环境检测与 `flutter doctor` 摘要
- [x] Android 环境检测（SDK / JDK / AVD）
- [x] iOS 环境检测（Xcode / CocoaPods / 模拟器）
- [x] 常用工具链检测（Python / Node.js / Git / Rust / Go / Java）
- [x] 总览页与侧边栏导航
- [x] Flutter 项目配置分析
- [x] 日志解析与错误统计
- [x] JSON / Markdown 诊断报告导出
- [x] macOS .app / .dmg 打包
- [x] 检测缓存与统一可执行文件发现
- [x] 深色模式、Cmd+K 命令面板与路径操作
- [x] 安装引导、日志增强、项目依赖更新检查
- [x] 报告历史对比与最近导出
- [x] 关于页、LICENSE 文件与开源许可展示
- [x] UI 细节优化（主次按钮、粘性页头、页面过渡、宽表滚动）
- [x] 统一 IPC 错误收敛与检测页状态机（`call()` / `useDetection`）

### 后续优化（未开始）

- [ ] GitHub Actions CI：自动执行 fmt / clippy / test / 前端构建
- [ ] 代码签名、公证、Universal Binary 与自动更新
- [ ] 日志错误跳转到编辑器对应行
- [ ] 应用日志落盘与「导出诊断日志」
- [ ] 收紧 CSP 与 Tauri capabilities 权限
- [ ] 关键页面 UI 冒烟测试与解析器 fixture 语料
- [ ] 中英文切换与无障碍优化
- [ ] CHANGELOG 与版本发布说明

## License

本项目基于 [MIT License](LICENSE) 开源。
