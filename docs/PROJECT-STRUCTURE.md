# DevScope 工程结构说明

这份文档解释项目里**每一个文件是干什么的**，以及**你以后该往哪写代码**。

---

## 一、整体是三层结构

```
┌─────────────────────────────────────────┐
│  UI 层 (Web 技术)                        │
│  src/*.vue  src/*.ts                    │
│  跑在 macOS 系统自带的 WebView 里        │
└─────────────────┬───────────────────────┘
                  │  invoke("命令名", 参数)   ← 前端 → Rust
                  │  Promise<返回值>          ← Rust → 前端
┌─────────────────▼───────────────────────┐
│  后端层 (Rust)                            │
│  src-tauri/src/*.rs                     │
│  真正的进程、文件、系统调用都在这里       │
└─────────────────┬───────────────────────┘
                  │  std::process  std::fs  std::env
┌─────────────────▼───────────────────────┐
│  macOS 系统                              │
└─────────────────────────────────────────┘
```

**核心概念**：UI 层没有权限碰系统。它想执行 `flutter --version`，必须 `invoke` 一个 Rust 命令，由 Rust 去执行、解析，再把结果以 JSON 形式传回来。

这就是"为什么这个项目值得做"的答案——权限边界强制你把业务逻辑放在 Rust 里，而不是散落在 JS 里。

---

## 二、根目录文件

| 文件 | 作用 | 要改吗 |
|---|---|---|
| `package.json` | 前端依赖清单 + 脚本命令 | 加前端库时改 |
| `package-lock.json` | npm 依赖的精确版本锁 | 自动生成，**要提交** |
| `index.html` | HTML 外壳，窗口加载的就是它 | 改窗口标题可动 |
| `vite.config.ts` | Vite 打包配置，端口锁死 1420 | 基本不用动 |
| `tsconfig.json` | TypeScript 编译规则（strict 已开） | 不用动 |
| `tsconfig.node.json` | 给 `vite.config.ts` 用的 TS 配置 | 不用动 |
| `.gitignore` | 哪些文件不进 Git | 加新产物时改 |
| `README.md` | 项目说明 | 功能变化时同步 |
| `AGENTS.md` | AI 开发规范（架构边界、编码约定） | 约定变化时改 |

### `index.html` 里的关键一行

```html
<div id="app"></div>
<script type="module" src="/src/main.ts"></script>
```

它只做两件事：占一个 `<div id="app">` 的位置，然后把 `src/main.ts` 加载进来。
**Vue 会把整个应用挂载到这个 div 上。** 所以你在 `index.html` 里写任何 HTML 都会被 Vue 覆盖掉，别在这写东西。

### `vite.config.ts` 里的关键三行

```ts
port: 1420,
strictPort: true,
ignored: ["**/src-tauri/**"],
```

- 端口固定 1420：因为 `tauri.conf.json` 里的 `devUrl` 写死了 `http://localhost:1420`
- `strictPort`：端口被占就直接报错，不会偷偷换成 1421 导致 Tauri 加载到空白页
- 忽略 `src-tauri`：防止你改 Rust 文件时 Vite 也去热重载一遍前端

---

## 三、`src/` —— UI 层（前端）

| 文件/目录 | 作用 |
|---|---|
| `src/main.ts` | **前端入口**。`createApp(App).mount("#app")` |
| `src/App.vue` | **根组件**，目前整个界面都在这里 |
| `src/vite-env.d.ts` | 类型声明，让 TS 认识 `.vue` 文件和 Vite 的环境变量 |
| `src/assets/` | 通过 `import` 引用的资源（会被 Vite 处理、加 hash） |
| `public/` | 原样拷贝到输出目录的静态文件（不处理、不加 hash） |

### `src/main.ts` 只有三行

```ts
import { createApp } from "vue";
import App from "./App.vue";

createApp(App).mount("#app");
```

这就是 Vue 应用的全部启动过程。**你几乎永远不用改它**（除非加路由、状态管理库）。

### `src/App.vue` 目前已清空为骨架

脚手架里的 Tauri Logo、输入框和 `greet` 示例已在 `dc898fc` 中删除，现在只剩一个居中标题和占位说明。

> 注意：前端已经不再调用 `invoke("greet")`，但 `lib.rs` 里的 `greet` 命令仍然注册着，等第一个真实命令（环境检测）写好后一起替换掉。

原来示例里这一行是**前端调用 Rust 的完整语法**，留着当语法参考：

```ts
async function greet() {
  greetMsg.value = await invoke("greet", { name: name.value });
}
```

注意第二个参数 `{ name: ... }` 的 key 名必须和 Rust 函数的参数名**完全一致**——这是最容易错的地方。

今后所有 `invoke` 调用都集中写在 `src/services/rust-api.ts`，组件里不直接调用。

> Vue 单文件组件（SFC）的三个块：`<script setup>` 写逻辑、`<template>` 写结构、`<style>` 写样式。

---

## 四、`src-tauri/` —— 后端层（Rust）

### 配置文件

| 文件 | 作用 | 要改吗 |
|---|---|---|
| `Cargo.toml` | **Rust 的依赖清单**，等价于 `package.json` | 加 crate 时改 |
| `Cargo.lock` | Rust 依赖的精确版本锁 | 自动生成，**要提交** |
| `build.rs` | 构建期脚本，编译前自动执行 | 不用动 |
| `tauri.conf.json` | 应用配置：窗口大小、标题、打包设置 | 要做 |
| `capabilities/default.json` | 权限声明：前端能调哪些系统能力 | 加插件时才改 |
| `rustfmt.toml` | 代码格式化配置（120 宽） | 不用动 |
| `icons/` | 应用图标（macOS 用 `.icns`） | 最后换掉 |

### `Cargo.toml` 里两个容易困惑的地方

**1）`[lib]` 那一块：**

```toml
[lib]
name = "devscope_lib"
crate-type = ["staticlib", "cdylib", "rlib"]
```

为什么既要有 `lib` 又要 `main.rs`？因为 Tauri 2 支持 iOS/Android，而移动端只能以库的形式被调用，不能有 `main` 函数。所以真正的逻辑放在 lib 里，`main.rs` 只负责"启动它"。

**2）`[profile.release]` 那一块：**

```toml
lto = true
opt-level = 3
strip = true
```

这是在追求**发布包体积最小**。你现在感受不到，第四周打包时才有意义。副作用是 `cargo build --release` 会慢很多。

### `.gitignore`（src-tauri 里那个）

忽略 `/target/`（编译产物，会涨到几个 G）和 `/gen/schemas`（自动生成的权限 schema）。

---

### 源码：`src-tauri/src/`

| 文件 | 作用 | 你写代码的地方 |
|---|---|---|
| `main.rs` | 二进制的 `main()`，只有 5 行 | ❌ 不用动 |
| `lib.rs` | **真正的程序入口 `run()`** | ✅ 经常动 |

#### `main.rs` —— 薄到不能再薄

```rust
fn main() {
    devscope_lib::run()
}
```

它只做一件事：把控制权交给 `lib.rs` 的 `run()`。第一行的 `#![cfg_attr(...)]` 是 Windows 下隐藏控制台窗口用的，你在 macOS 上可以完全无视，但**不要删**。

**为什么要这么绕？** 见上面 `crate-type` 的解释——移动端没有 `main()`，所以逻辑必须在 lib 里。

#### `lib.rs` —— 你的主战场

```rust
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

这一段是整个架构的**缩影**，逐行拆解：

| 代码 | 含义 |
|---|---|
| `#[tauri::command]` | 宏，把这个普通函数**注册成一个可被前端调用的命令** |
| `fn greet(name: &str) -> String` | 参数来自前端 JSON，返回值被序列化成 JSON |
| `tauri::Builder::default()` | 构建器模式（Builder Pattern），链式配置 |
| `.plugin(...)` | 加载插件 |
| `.invoke_handler(generate_handler![greet])` | **命令白名单**——没登记的 `#[tauri::command]` 前端调不到 |
| `.run(generate_context!())` | 启动事件循环，`generate_context!` 在编译期读取 `tauri.conf.json` 并嵌入资源 |
| `.expect(...)` | 启动失败直接崩，因为这时没有 UI 可以报错 |

**注意 `generate_handler![greet]` 这一行**：以后每加一个命令，都必须在这里补一句，否则前端调用会报 `command not found`。这是新手最常见的坑之一。

---

## 五、一次完整调用发生了什么

以脚手架自带的 `greet` 为例：

```
① App.vue:  await invoke("greet", { name: "张飞" })
                    ↓
② Tauri 运行时把参数序列化成 JSON:  { "name": "张飞" }
                    ↓
③ IPC 跨进程传到 Rust 侧（WebView → Rust 主进程）
                    ↓
④ 反序列化成 Rust 的 &str，调用 fn greet(name: &str)
                    ↓
⑤ 函数返回 String:  "Hello, 张飞! You've been greeted from Rust!"
                    ↓
⑥ 序列化成 JSON 字符串，传回 WebView
                    ↓
⑦ Promise resolve，赋给 greetMsg.value，界面自动更新
```

**你要练的就是第 ④ 步**：在 Rust 里做真正的工作（跑进程、读文件、解析），然后返回结构化的数据。

---

## 六、以后该在哪写什么（速查表）

现在一共只有 5 个源文件（前端 3 个、Rust 2 个），等按计划推进后会长成这样：

```
src/                          ← UI
├── main.ts                   入口（不改）
├── App.vue                   根组件（放侧边栏 + 路由出口）
├── views/                    ★ 整页 : Dashboard.vue / Environment.vue ...
├── components/               ★ 可复用小组件 : StatusBadge.vue ...
├── services/rust-api.ts      ★ 所有 invoke 调用集中在这，别散在组件里
└── types/index.ts            ★ 和 Rust 结构体一一对应的 TS interface

src-tauri/src/                ← Rust
├── main.rs                   入口（不改）
├── lib.rs                    注册命令、启动 Builder
├── commands/                 ★ 薄薄一层：接收参数 → 调 service → 返回
├── services/                 ★ 真正干活的地方，可单独测试
├── models/                   ★ struct / enum 定义，前后端的数据契约
└── utils/                    ★ 通用小工具
```

### 判断标准

| 你想做的事 | 写在哪 |
|---|---|
| 加一个新的前端可调用命令 | `src-tauri/src/commands/xxx.rs` + 在 `lib.rs` **注册** |
| 写跑 `flutter --version` 这类逻辑 | `src-tauri/src/services/` |
| 定义返回给前端的结构体 | `src-tauri/src/models/` |
| 加一个新页面 | `src/views/Xxx.vue` |
| 写一个带状态颜色的小标签 | `src/components/Xxx.vue` |
| 改窗口标题/大小 | `src-tauri/tauri.conf.json` |

**一个原则**：`commands/` 里只写"转发"，逻辑放 `services/`。这样第三周写单元测试时，你可以直接测 service，不用启动整个 Tauri。

---

## 七、绝对不要手改的文件

| 路径 | 原因 |
|---|---|
| `node_modules/` | npm 自动管理，96MB |
| `src-tauri/target/` | Cargo 编译产物，已 2GB，删了会重新编译 |
| `src-tauri/gen/` | Tauri 自动生成的权限 schema |
| `dist/` | 前端构建输出 |
| `package-lock.json` / `Cargo.lock` | 只让工具改，但**必须提交到 Git** |

---

## 八、两个 `gitignore` 的分工

- **根目录 `.gitignore`** —— 管前端：`node_modules`、`dist`、`.DS_Store`
- **`src-tauri/.gitignore`** —— 管 Rust：`target`、`gen/schemas`

不用合并，各管各的。
