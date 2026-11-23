# DevScope 工程结构说明

这份文档解释项目里**每个目录和关键文件是干什么的**，以及新增功能时该往哪写。
架构规则以 [AGENTS.md](../AGENTS.md) 为准，这里更偏向「读代码时的地图」。

---

## 一、整体是三层结构

```
┌─────────────────────────────────────────┐
│  UI 层 (Web 技术)                        │
│  src/**/*.vue  src/**/*.ts              │
│  跑在 macOS 系统自带的 WebView 里        │
└─────────────────┬───────────────────────┘
                  │  invoke("命令名", 参数)   ← 前端 → Rust
                  │  Promise<返回值>          ← Rust → 前端
┌─────────────────▼───────────────────────┐
│  后端层 (Rust)                            │
│  src-tauri/src/**/*.rs                  │
│  真正的进程、文件、系统调用都在这里       │
└─────────────────┬───────────────────────┘
                  │  std::process  std::fs  std::env
┌─────────────────▼───────────────────────┐
│  macOS 系统                              │
└─────────────────────────────────────────┘
```

前端没有权限碰系统。它想执行 `flutter --version`，必须 `invoke` 一个 Rust 命令；
Rust 执行、解析，再把结构化 JSON 传回来。前端只负责展示和交互。

---

## 二、当前目录树

```
devscope/
├── index.html
├── package.json
├── package-lock.json
├── vite.config.ts
├── tsconfig.json
├── AGENTS.md
├── README.md
├── docs/
│   └── PROJECT-STRUCTURE.md
├── src/                              前端
│   ├── main.ts
│   ├── App.vue
│   ├── router/index.ts
│   ├── views/
│   │   ├── Overview.vue
│   │   ├── FlutterEnvironment.vue
│   │   ├── AndroidEnvironment.vue
│   │   └── IosEnvironment.vue
│   ├── components/
│   │   ├── StatusBadge.vue
│   │   ├── IssueList.vue
│   │   └── EnvironmentRow.vue
│   ├── services/
│   │   ├── rust-api.ts
│   │   └── status.ts
│   ├── types/index.ts
│   ├── styles/app.css
│   └── vite-env.d.ts
└── src-tauri/                        Rust
    ├── Cargo.toml
    ├── tauri.conf.json
    ├── capabilities/default.json
    ├── icons/                        应用图标 / 菜单栏图标
    └── src/
        ├── main.rs
        ├── lib.rs
        ├── tray.rs
        ├── commands/
        │   ├── mod.rs
        │   └── environment.rs
        ├── services/
        │   ├── mod.rs
        │   ├── flutter.rs
        │   ├── flutter/doctor.rs
        │   ├── android.rs
        │   ├── ios.rs
        │   └── system.rs
        ├── models/
        │   ├── mod.rs
        │   ├── environment.rs
        │   ├── doctor.rs
        │   ├── android.rs
        │   ├── ios.rs
        │   └── system.rs
        └── utils/
            ├── mod.rs
            ├── fs.rs
            ├── process.rs
            └── version.rs
```

---

## 三、前端 `src/`

### `main.ts`

应用入口，只做三件事：引入全局样式、挂载路由、挂载 `App`。

```ts
createApp(App).use(router).mount("#app");
```

### `App.vue`

应用外壳：左侧品牌与导航，右侧 `<RouterView />`。
页面本身不实现侧边栏；导航高亮由 Vue Router 的 `active-class` 负责。
页面组件用 `<KeepAlive>` 缓存，返回时不重复执行慢检测；每个详情页都有刷新按钮。

### `router/index.ts`

| 路径 | 页面 |
|---|---|
| `/` | 总览 |
| `/environment/flutter` | Flutter |
| `/environment/android` | Android |
| `/environment/ios` | iOS / Xcode |

使用 `createWebHashHistory()`。Tauri WebView 用自定义协议加载静态文件，没有
history fallback；hash 路由在刷新时不会因为找不到 `/environment/flutter` 而白屏。

### `views/`

| 页面 | 职责 |
|---|---|
| `Overview.vue` | 展示本机配置、常用开发环境与语言；点击跳到对应环境页 |
| `FlutterEnvironment.vue` | Flutter SDK 检测 + 按需运行 `flutter doctor -v` |
| `AndroidEnvironment.vue` | Android SDK / JDK / AVD 详情 |
| `IosEnvironment.vue` | Xcode / CocoaPods / Swift / 模拟器详情 |

页面统一处理 loading / IPC 失败 / 未安装 / 业务错误 / 正常（含 warning）几种状态。

### `components/`

| 组件 | 职责 |
|---|---|
| `StatusBadge.vue` | 状态标签，tone 为 `ok` / `warning` / `error` |
| `IssueList.vue` | Android / iOS 的可修复问题列表 |
| `EnvironmentRow.vue` | 总览页的开发环境行，整行可跳转 |

组件只展示，不调用 IPC。

### `services/`

| 文件 | 职责 |
|---|---|
| `rust-api.ts` | 所有 `invoke()` 调用集中在这里 |
| `status.ts` | 业务状态 → `StatusBadge` tone / 中文标签的统一映射 |

所有 `invoke()` 集中在 `rust-api.ts`：

```ts
detectFlutterEnvironment()   → detect_flutter_environment
runFlutterDoctor()           → run_flutter_doctor
detectAndroidEnvironment()   → detect_android_environment
detectIosEnvironment()       → detect_ios_environment
detectSystemInfo()           → detect_system_info
```

命令名、参数名必须和 Rust 侧逐字一致。业务失败（未安装、缺组件）不 reject，
通过返回值的 `status` / `issues` / `message` 表达；只有 IPC 或任务机制失败才 reject。

### `types/index.ts`

与 Rust 模型一一对应的 TS interface。Rust 侧使用 `#[serde(rename_all = "camelCase")]`，
所以这里字段全部是 camelCase；`Option<T>` 对应 `T | null`。

### `styles/app.css`

设计令牌和跨页面复用的样式：卡片、指标、详情行、状态卡、按钮、spinner、
标签、响应式断点。页面级特有样式仍写在 `.vue` 的 `<style scoped>` 里。

---

## 四、Rust `src-tauri/src/`

### `main.rs`

只有 `devscope_lib::run()`。不要动。

### `lib.rs`

应用初始化与命令注册。**每新增一个 `#[tauri::command]`，必须在这里的
`generate_handler![...]` 中注册**，否则前端报 `command not found`。
菜单栏图标在 `setup` 里交给 `tray::init`。

### `tray.rs`

菜单栏常驻图标：托盘图标 + 菜单（显示 DevScope / 关于 / 退出），
在 `lib.rs` 的 `setup` 中调用一次。

图标规则和桌面图标完全不是一套，换图前先看这里：

- 只用一张 `icons/tray.png`，**44×44 px**。Tauri 会把图标高度**归一成 18pt**
  （`tray-icon` 的内部实现，`set_icon_for_ns_status_item_button`），
  44px 正好是 18pt 的 2x；换成 512 的图不会更大，只会让二进制变大——
  图标是按原始像素编进可执行文件的。
- 必须是纯黑 + alpha 的模板图，并开 `icon_as_template(true)`，
  由 macOS 自动反色适配浅色 / 深色菜单栏。
- 图形要撑满画布，只留 2px 安全边：整张图的高度都会映射成 18pt，留白越多图形越小。
- 当前图形是 SF Pro Bold 的「D」单字母，18pt 下仍可辨认；不要塞 `</>`、放大镜等细节。

「关于」面板刻意不传 `icon`，直接用 bundle 里的 `icon.icns`，和 Dock 保持一致。
（`muda` 对自定义 icon 不做缩放，像素尺寸会被当成 pt 尺寸。）

### `icons/`

`src-tauri/icons/`（不在 `src/` 下）：应用图标与菜单栏图标。

| 文件 | 用途 |
|---|---|
| `source-1024.png` | 图标母版：1024×1024，内容 824×824，外圈全透明 |
| `icon.icns` | 桌面图标（Dock / Finder / Launchpad / Cmd-Tab），含 16~1024 全部 10 档 |
| `tray.png` | 菜单栏图标，44×44 纯黑模板图 |
| `32x32.png`、`64x64.png`、`128x128.png`、`128x128@2x.png`、`icon.png` | 非 macOS 平台与窗口图标 |
| `icon.ico`、`Square*Logo.png`、`StoreLogo.png` | Windows 专用，macOS 用不到 |
| `android/`、`ios/` | 移动端图标，当前不发布移动端 |

重新生成桌面图标：

```bash
npx tauri icon src-tauri/icons/source-1024.png --output src-tauri/icons
```

注意：`tauri icon` 会顺带**覆盖 `tray.png`**，并按亮度自动抠一张模板图 ——
那个结果在 18pt 下是一团黑块，不能直接用。覆盖后从 Git 恢复：

```bash
git checkout src-tauri/icons/tray.png
```

macOS 图标网格：画布 1024，内容圆角方形 824×824（四周各留 100px），
圆角约 185.4px，用超椭圆；外圈不要投影。

### `commands/environment.rs`

薄转发层：把阻塞检测丢到 `spawn_blocking`，把机制性失败转成字符串。
当前五个命令：

| 命令 | 服务 |
|---|---|
| `detect_flutter_environment` | `services::flutter::detect` |
| `run_flutter_doctor` | `services::flutter::run_flutter_doctor` |
| `detect_android_environment` | `services::android::detect` |
| `detect_ios_environment` | `services::ios::detect` |
| `detect_system_info` | `services::system::detect` |

### `services/`

真正的业务逻辑，全部可以在不启动 Tauri 的情况下用 `cargo test` 测试。

| 文件 | 职责 |
|---|---|
| `flutter.rs` | 查找 flutter（PATH → 常见目录 → 登录 shell）、执行 `--version --machine`、解析版本；包含 `doctor` 子模块 |
| `flutter/doctor.rs` | 执行并解析 `flutter doctor -v`，处理 verbose 前缀，保留可读报告 |
| `android.rs` | 读取 SDK 目录、JDK `release` 文件、licenses、AVD；不启动 adb / emulator |
| `ios.rs` | `xcode-select`、`xcodebuild`、`xcrun simctl`、`swift`、`pod`，带超时执行 |
| `system.rs` | `sw_vers`、`sysctl`、`uname`、`scutil`，读取 macOS / 芯片 / 内存 / 机型 |

关键设计：

- 业务失败不返回 `Err`，而是收敛到模型的 `status` / `message` / `issues`。
- 外部命令统一走 `utils::process::run_with_timeout`，有超时、输出上限和退出码语义。
- 不自动执行安装、接受 license、修改环境等操作，只展示建议命令。
- 测试不依赖本机安装 Flutter / Xcode / Android SDK：用假脚本、临时目录和固定输出。

### `models/`

前后端数据契约，全部 `#[derive(Serialize)]`。

| 文件 | 模型 |
|---|---|
| `environment.rs` | `FlutterStatus`、`FlutterExecutableSource`、`FlutterVersionInfo`、`FlutterEnvironment` |
| `doctor.rs` | `DoctorStatus`、`DoctorSection`、`DoctorReport` |
| `android.rs` | `AndroidStatus`、`AndroidSdkSource`、`JavaSource`、`JavaRuntime`、`AndroidEnvironment` |
| `ios.rs` | `IosStatus`、`CocoaPodsSource`、`CocoaPodsInfo`、`SimulatorInfo`、`IosEnvironment` |
| `system.rs` | `SystemInfo` |

模型按检测对象拆分，不把 Android / iOS 字段塞进 `FlutterEnvironment`。
总览页的聚合在 UI 层完成，避免后端出现 God model。

### `utils/`

| 文件 | 职责 |
|---|---|
| `fs.rs` | `home_dir`、`is_executable`（三个检测器共用） |
| `process.rs` | `run_with_timeout`、输出上限、`ProcessOutput` / `ProcessError` |
| `version.rs` | 按数字段比较版本，例如 `android-37.0` > `android-36` |

`utils` 只放通用小工具，不要变成杂物间。

---

## 五、一次检测的数据流

以 Android 为例：

```
① Overview.vue / AndroidEnvironment.vue
       await detectAndroidEnvironment()
                    ↓
② rust-api.ts: invoke("detect_android_environment")
                    ↓
③ commands/environment.rs: spawn_blocking(android::detect)
                    ↓
④ services/android.rs
       - 读取 ANDROID_HOME / ANDROID_SDK_ROOT / 常见目录
       - 扫描 platforms、build-tools、cmdline-tools、licenses
       - 查找 JDK、adb、emulator、AVD
                    ↓
⑤ AndroidEnvironment { status, issues, ... }
                    ↓
⑥ serde 序列化为 camelCase JSON，Promise resolve
                    ↓
⑦ 页面按 status / issues 渲染
```

---

## 六、各模块的检测口径

### 本机配置

- 系统版本：`sw_vers`（ProductName / ProductVersion / BuildVersion）。
- 架构：`uname -m`。
- 机型、芯片、核心数、内存：`sysctl -n hw.model hw.ncpu hw.memsize machdep.cpu.brand_string` 对应的单项查询。
- 电脑名称：`scutil --get ComputerName`，失败时退回 `hostname`。
- 每个字段独立可选；某条命令失败时只把该字段留空，不影响总览其他内容。

### Flutter

- 路径优先级：GUI PATH → 常见安装目录 → 登录 shell 的 `command -v flutter`。
- 版本命令：`flutter --no-version-check --version --machine`。
- doctor：`flutter --no-version-check doctor -v`，60 秒超时，按需触发。
- doctor 的 `-v` 是全局 verbose，输出每行带 `[  +10 ms]` 前缀；解析前先剥离。
- doctor 是 Flutter 视角的摘要，Android / iOS 详情页才是各自模块的权威。

### Android

- SDK：环境变量 → `~/Library/Android/sdk` → `~/Android/Sdk` → Homebrew 目录。
- JDK：`JAVA_HOME` → Android Studio JBR → SDKMAN / Homebrew / 系统 JVM。
- 版本：读取 JDK 的 `release` 文件，不启动 `java -version`。
- 模拟器：读取 `~/.android/avd/*.ini`，不启动 `emulator -list-avds`。
- licenses：统计 `licenses/` 下的文件，不读取内容、不代执行 `sdkmanager --licenses`。

### iOS / Xcode

- 开发者目录：`xcode-select -p`。
- Xcode：`/usr/bin/xcodebuild -version`；识别许可协议未接受的错误。
- Command Line Tools：检查 `/Library/Developer/CommandLineTools`。
- CocoaPods：PATH → RubyGems / rbenv / rvm / Homebrew → 登录 shell；执行 `pod --version`。
- 模拟器：`xcrun simctl list devices available --json`，只统计 iOS runtime。
- Swift：`swift --version`。

所有命令都有超时；检测失败不会 kill 用户的其他进程，也不会修改 Xcode 配置。

---

## 七、新增一个环境模块时

1. `models/xxx.rs`：定义状态枚举和结果 struct，加 `Serialize` + camelCase。
2. `services/xxx.rs`：实现 `detect()`，把可测试的解析逻辑拆成纯函数，补单测。
3. `commands/environment.rs`：加薄命令，`spawn_blocking` 调 service。
4. `lib.rs`：注册命令。
5. `src/types/index.ts`：写对应 interface。
6. `src/services/rust-api.ts`：加 invoke 封装。
7. `src/views/XxxEnvironment.vue`：处理 loading / 未安装 / 错误 / warning。
8. `src/router/index.ts` + `App.vue`：加路由和导航项。
9. 更新本文档和 README。

---

## 八、验证命令

```bash
# Rust（在仓库根目录执行）
~/.cargo/bin/cargo fmt --manifest-path src-tauri/Cargo.toml --check
~/.cargo/bin/cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
~/.cargo/bin/cargo test --manifest-path src-tauri/Cargo.toml

# 前端（含 vue-tsc 类型检查）
npm run build

# 桌面端真实联调
npm run tauri dev
```

在 DSH / Codex 等非登录 shell 中，`cargo` 可能不在 PATH；先
`export PATH="$HOME/.cargo/bin:$PATH"`。

---

## 九、绝对不要手改的文件

| 路径 | 原因 |
|---|---|
| `node_modules/` | npm 自动管理 |
| `src-tauri/target/` | Cargo 编译产物 |
| `src-tauri/gen/` | Tauri 自动生成的权限 schema |
| `dist/` | 前端构建输出 |
| `package-lock.json` / `Cargo.lock` | 只让工具改，但要提交进 Git |
