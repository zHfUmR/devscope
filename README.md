# DevScope

> macOS 开发者工具箱 —— 本地开发环境诊断与项目分析。

一个用 **Rust + Tauri 2** 构建的原生 macOS 桌面应用，用于检测本机开发环境（Flutter / Dart / Xcode / Git / Python / Rust）、分析 Flutter 项目配置、解析构建日志并生成诊断报告。

## 状态

🚧 **开发中** —— 项目处于初始骨架阶段，功能正在逐步实现。

## 技术栈

| 层 | 技术 |
|---|---|
| 桌面框架 | Tauri 2 |
| 后端 | Rust (edition 2024) |
| 前端 | Vue 3 + TypeScript |
| 构建 | Vite 8 |
| 目标平台 | macOS (Apple Silicon) |

## 环境要求

- macOS 12+
- Rust 1.80+（[rustup](https://rustup.rs)）
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
├── src/                前端（Vue 3 + TypeScript）
└── src-tauri/          Rust 后端
    └── src/
        ├── commands/   前端可调用的命令（薄转发层）
        ├── services/   业务逻辑
        └── models/     数据结构
```

详细的逐文件说明见 [docs/PROJECT-STRUCTURE.md](docs/PROJECT-STRUCTURE.md)。

## 开发路线

- [x] 工程初始化（Tauri 2 + Vue 3 + TypeScript）
- [ ] 开发环境检测（Flutter / Dart / Xcode / Git / Python / Rust）
- [ ] Flutter 项目配置分析
- [ ] 日志解析与错误统计
- [ ] JSON / Markdown 诊断报告导出
- [ ] macOS .app / .dmg 打包

## License

MIT
