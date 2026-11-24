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

🚧 **开发中** —— 项目功能正在逐步实现。

</div>

---

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
| 工程骨架 | Tauri 2 + Vue 3 + TypeScript | 已完成 |

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
├── src/                         前端（Vue 3 + TypeScript）
│   ├── router/                  路由（hash history）
│   ├── views/                   总览 / 环境检测 / 项目 / 日志 / 报告
│   ├── components/              可复用展示组件
│   ├── services/                rust-api、原生对话框、错误与状态映射
│   ├── types/index.ts           与 Rust 模型一一对应的 TS 类型
│   └── styles/app.css           设计令牌与共享样式
└── src-tauri/                   Rust 后端
    └── src/
        ├── tray.rs              菜单栏常驻图标与菜单
        ├── commands/            前端可调用的命令（薄转发层）
        ├── services/           环境检测 + 项目 / 日志 / 报告分析
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
