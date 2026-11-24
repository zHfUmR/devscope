# AGENTS.md — DevScope 开发规范

本文件是 DevScope 的项目级**开发规则**，供 AI 编程助手（Codex / DeepSeek Harness / Claude Code）和人类贡献者遵守。
它只写「怎么做、不能做什么」；项目介绍见 [README.md](README.md)，逐文件说明见 [docs/PROJECT-STRUCTURE.md](docs/PROJECT-STRUCTURE.md)。

## 1. 项目与技术栈

DevScope 是一个 macOS 原生开发者工具箱：检测本机开发环境（Flutter / Dart / Xcode / Git / Python / Rust）、分析 Flutter 项目、解析构建日志并导出诊断报告。

- 桌面框架：Tauri 2
- 后端：Rust（edition 2024）——**核心业务层**
- 前端：Vue 3 + TypeScript（Composition API）+ Vue Router
- 构建：Vite
- 包管理：npm
- 目标平台：macOS（先 Apple Silicon）

项目处于初始阶段。优先保证**正确性、可维护性和学习价值**，而不是功能数量。

## 2. 架构边界（最重要）

三层，权限逐级向下：

```
Vue 3 + TypeScript      UI、交互、状态、展示
        │  invoke()  ← Tauri IPC
Rust (Tauri 2)          业务逻辑、进程、文件、环境变量
        │  std::process / std::fs / std::env
macOS                   系统能力
```

硬性规则：

- 系统级操作（执行命令、读文件、探测环境变量、扫描目录）**只能写在 Rust**。
- Vue **不得**直接执行系统命令或实现环境检测逻辑；它只负责展示和交互。
- 前后端只通过 Tauri IPC 通信。
- `commands/` 保持薄：接收参数 → 调 service → 返回。不写业务逻辑。
- 业务逻辑必须能在**不启动 Tauri** 的情况下用 `cargo test` 测试。
- 不为「以后可能用到」提前抽象，也不预留空目录。

## 3. 目录职责

**当前实际状态**：Flutter / Android / iOS / 常用工具链检测、30 秒检测缓存、深色模式与 Cmd+K 命令面板，以及 Flutter 项目分析 / 构建日志分析 / 诊断报告已经落地；`views/`、`components/`、`services/`、`types/`、`router/` 均已有实际代码。新增目录只在有实际代码时创建，不为「以后可能用到」预铺空目录。

前端 `src/`：

| 路径 | 职责 |
|---|---|
| `main.ts` | 入口，挂载 `App` 与路由（基本不动） |
| `App.vue` | 根布局：侧边栏 + 路由出口 |
| `router/` | 路由表与 hash history |
| `views/` | 整页组件 `Overview.vue` / `FlutterEnvironment.vue` … |
| `components/` | 可复用 UI 组件 |
| `services/rust-api.ts` | 所有 `invoke` 调用集中在此 |
| `types/index.ts` | 与 Rust 结构体一一对应的 TS interface |
| `styles/app.css` | 设计令牌与跨页面共享样式 |

Rust `src-tauri/src/`：

| 路径 | 职责 |
|---|---|
| `main.rs` | 二进制入口，只调用 `run()`（不动） |
| `lib.rs` | 应用初始化、插件与命令注册（不放业务逻辑） |
| `tray.rs` | 菜单栏常驻图标与菜单（应用装配，不放检测逻辑） |
| `commands/` | Tauri 命令：参数进出 + 转发 |
| `services/` | 真正的业务逻辑，可独立测试 |
| `models/` | struct / enum，前后端数据契约 |
| `utils/` | 通用小工具，不要变成杂物间 |

## 4. Rust 规范

- 写 idiomatic Rust，遵循 edition 2024。
- 用 `Result<T, E>` 处理可恢复错误，`Option<T>` 表示缺失值。
- 业务逻辑里避免 `.unwrap()` / `.expect()`；启动路径上的致命错误可以 `expect` 并写清原因。
- 结构化数据统一用 `serde` / `serde_json`。
- 领域错误类型需要时再引入 `thiserror`；应用层聚合错误需要时再引入 `anyhow`。加依赖前先说明理由。
- 函数短小、职责单一；先考虑标准库，再考虑新 crate。
- 核心逻辑要有单元测试（`#[cfg(test)]`），测试不能依赖本机已安装 Flutter/Xcode——把「查找二进制」「解析版本字符串」拆成可测的纯函数。

## 5. 进程执行安全（macOS）

- **禁止**把用户输入拼进 shell 字符串；用 `std::process::Command`，参数分开传。
- 给外部命令设置超时；处理三种失败：可执行文件不存在、非零退出码、输出非 UTF-8 / 不可解析。
- 读取进程输出要有上限，避免一次性吞入无界输出。
- **GUI 应用不会继承终端的 PATH**。查 `flutter`、`xcodebuild` 前必须显式探测常见安装路径或走用户登录 shell，不能假设和终端环境一致。
- 绝不自动修改用户的开发环境。

## 6. Vue / TypeScript 规范

- 统一 `<script setup lang="ts">`。
- TS strict 已开启，不要用 `any`；确需时写注释说明原因。
- 页面组件只负责组合 UI；可复用逻辑抽到 `services/` 或 composable。
- IPC 调用只能出现在 `src/services/`，不要散落在组件里。
- 必须处理 loading / success / empty / error 四种状态。
- 不要在前端重复实现 Rust 的业务逻辑。

## 7. 前后端 IPC 约定

- 前端用 `invoke()`，来自 `@tauri-apps/api/core`。
- Rust 命令名、参数名与 TS 调用侧**逐字一致**（参数在 TS 里是对象的 key）。
- Rust 侧返回显式模型（`#[derive(Serialize)]`），TS 侧在 `src/types/` 写对应 interface。
- 尽量返回结构化字段，而不是拼好的展示字符串；格式化交给前端。
- 每新增一个 `#[tauri::command]`，**必须**在 `lib.rs` 的 `generate_handler![...]` 里注册，否则前端报 `command not found`。
- 后端错误要转成用户可读的信息，不要静默吞掉。

## 8. 开发流程与验证

动手前：先读现有结构与约定，确认最小改动范围；涉及架构决策时先简述方案。

完成后必须实际执行并汇报结果：

```bash
# Rust（在仓库根目录执行；Cargo.toml 在 src-tauri/）
~/.cargo/bin/cargo fmt --manifest-path src-tauri/Cargo.toml --check
~/.cargo/bin/cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
~/.cargo/bin/cargo test --manifest-path src-tauri/Cargo.toml

# 前端
npm run build      # 含 vue-tsc 类型检查
```

- 在 DSH / Codex 等非登录 shell 中，`cargo` 可能不在 PATH；需要时先 `export PATH="$HOME/.cargo/bin:$PATH"`，不要因此判定「Rust 未安装」。
- **没有实际跑过的测试，不许声称通过。** 汇报里写清改了哪些文件、怎么实现的、测试结果、已知限制。

## 9. 范围控制（非目标）

- MVP 阶段不接入 AI、云同步、远程服务，不自动修改用户环境。
- 新依赖必须有明确理由；优先用已有依赖。
- 不引入没有具体需求的复杂架构。
- 首个发布版本聚焦：本地环境诊断 + Flutter 项目检查。

## 10. 禁止事项

- 不手改 `node_modules/`、`src-tauri/target/`、`src-tauri/gen/`、`dist/`。
- 不手改 `Cargo.lock`、`package-lock.json`（只让工具改，但必须提交进 Git）。
- 不硬编码本机绝对路径。
- 不执行破坏性命令。
- 未经明确要求，不执行 `git commit` / `git push`。
- 不用占位实现冒充已完成功能，不静默忽略错误。
- 架构、命令或用户可见行为变更时，同步更新文档。
