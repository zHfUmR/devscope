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
│   │   ├── IosEnvironment.vue
│   │   ├── Toolchains.vue
│   │   ├── ProjectAnalysis.vue
│   │   ├── LogAnalysis.vue
│   │   └── DiagnosticReport.vue
│   ├── components/
│   │   ├── StatusBadge.vue
│   │   ├── IssueList.vue
│   │   ├── EnvironmentRow.vue
│   │   ├── DetailList.vue
│   │   ├── CommandPalette.vue
│   │   ├── CopyButton.vue
│   │   └── PathActions.vue
│   ├── services/
│   │   ├── rust-api.ts
│   │   ├── dialog.ts
│   │   ├── errors.ts
│   │   ├── clipboard.ts
│   │   ├── install-guide.ts
│   │   ├── reveal.ts
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
        │   ├── environment.rs
        │   └── analysis.rs
        ├── services/
        │   ├── mod.rs
        │   ├── flutter.rs
        │   ├── flutter/doctor.rs
        │   ├── android.rs
        │   ├── ios.rs
        │   ├── system.rs
        │   ├── toolchain.rs
        │   ├── cache.rs
        │   ├── project.rs
        │   ├── project/pubspec.rs
        │   ├── project/gradle.rs
        │   ├── project/ios_project.rs
        │   ├── build_log.rs
        │   ├── outdated.rs
        │   ├── report.rs
        │   └── report/markdown.rs
        ├── models/
        │   ├── mod.rs
        │   ├── environment.rs
        │   ├── doctor.rs
        │   ├── android.rs
        │   ├── ios.rs
        │   ├── system.rs
        │   ├── toolchain.rs
        │   ├── outdated.rs
        │   ├── project.rs
        │   ├── build_log.rs
        │   └── report.rs
        └── utils/
            ├── mod.rs
            ├── executable.rs
            ├── fs.rs
            ├── process.rs
            ├── time.rs
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
应用还挂载了 Cmd+K 命令面板，并提供跟随 macOS 外观的深色模式（见 `styles/app.css` 的令牌覆盖）。

### `router/index.ts`

| 路径 | 页面 |
|---|---|
| `/` | 总览 |
| `/environment/flutter` | Flutter |
| `/environment/android` | Android |
| `/environment/ios` | iOS / Xcode |
| `/environment/toolchains` | 常用工具链 |
| `/project` | Flutter 项目分析 |
| `/logs` | 构建日志分析 |
| `/report` | 诊断报告 |

使用 `createWebHashHistory()`。Tauri WebView 用自定义协议加载静态文件，没有
history fallback；hash 路由在刷新时不会因为找不到 `/environment/flutter` 而白屏。

### `views/`

| 页面 | 职责 |
|---|---|
| `Overview.vue` | 展示本机配置、常用开发环境与语言；点击跳到对应环境页 |
| `FlutterEnvironment.vue` | Flutter SDK 检测 + 按需运行 `flutter doctor -v` |
| `AndroidEnvironment.vue` | Android SDK / JDK / AVD 详情 |
| `IosEnvironment.vue` | Xcode / CocoaPods / Swift / 模拟器详情 |
| `Toolchains.vue` | Python / Node.js / Git / Rust / Go / Java 的路径与版本 |
| `ProjectAnalysis.vue` | 选择项目目录，展示 pubspec / 依赖 / Android / iOS 配置 |
| `LogAnalysis.vue` | 选择构建日志，展示错误 / 警告分组与构建结果 |
| `DiagnosticReport.vue` | 生成、预览并导出 JSON / Markdown 诊断报告 |

页面统一处理 loading / IPC 失败 / 未选择或空状态 / 业务错误 / 正常（含 warning）几种状态。

### `components/`

| 组件 | 职责 |
|---|---|
| `StatusBadge.vue` | 状态标签，tone 为 `ok` / `warning` / `error` |
| `IssueList.vue` | Android / iOS 的可修复问题列表 |
| `EnvironmentRow.vue` | 总览页的开发环境行，整行可跳转 |
| `DetailList.vue` | 通用「标签 / 值」详情列表，供项目分析和报告页复用 |
| `CommandPalette.vue` | Cmd+K 快速跳转面板 |
| `CopyButton.vue` | 通用复制按钮，带成功 / 失败反馈 |
| `PathActions.vue` | 路径操作：复制 + 在 Finder 中显示 |

组件只展示，不调用 IPC。

### `services/`

| 文件 | 职责 |
|---|---|
| `rust-api.ts` | 所有 `invoke()` 调用集中在这里 |
| `dialog.ts` | 原生目录 / 文件 / 保存对话框（`@tauri-apps/plugin-dialog`） |
| `errors.ts` | 未知错误 → 用户可读字符串 |
| `clipboard.ts` | 复制文本，兼容 Clipboard API 与旧接口 |
| `install-guide.ts` | 工具链安装命令与下载页（只展示，不执行） |
| `reveal.ts` | 在 Finder 中定位文件 / 打开外部链接（opener 插件） |
| `status.ts` | 业务状态 → `StatusBadge` tone / 中文标签的统一映射 |

所有 `invoke()` 集中在 `rust-api.ts`：

```ts
detectFlutterEnvironment()   → detect_flutter_environment
runFlutterDoctor()           → run_flutter_doctor
detectAndroidEnvironment()   → detect_android_environment
detectIosEnvironment()       → detect_ios_environment
detectSystemInfo()           → detect_system_info
detectToolchains()           → detect_toolchains
analyzeFlutterProject(path)  → analyze_flutter_project
analyzeBuildLog(path)        → analyze_build_log
generateDiagnosticReport()   → generate_diagnostic_report
saveDiagnosticReport(...)    → save_diagnostic_report
renderDiagnosticReport(...)  → render_diagnostic_report
loadDiagnosticReport(path)   → load_diagnostic_report
checkFlutterOutdated(path)   → check_flutter_outdated
saveLogExport(path, text)    → save_log_export
printCurrentWindow()         → print_current_window
```

命令名、参数名必须和 Rust 侧逐字一致。业务失败（未安装、缺组件、不是 Flutter
项目、日志读不到）不 reject，通过返回值的 `status` / `issues` / `message` 表达；
只有 IPC 或任务机制失败才 reject。

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
当前六个命令：

| 命令 | 服务 |
|---|---|
| `detect_flutter_environment` | `services::flutter::detect` |
| `run_flutter_doctor` | `services::flutter::run_flutter_doctor` |
| `detect_android_environment` | `services::android::detect` |
| `detect_ios_environment` | `services::ios::detect` |
| `detect_system_info` | `services::system::detect` |
| `detect_toolchains` | `services::toolchain::detect_all` |

### `commands/analysis.rs`

项目分析、日志分析与报告导出的薄转发层：

| 命令 | 服务 |
|---|---|
| `analyze_flutter_project(path)` | `services::project::analyze` |
| `analyze_build_log(path)` | `services::build_log::analyze` |
| `generate_diagnostic_report(projectPath?, logPath?, force?)` | `services::report::generate` |
| `save_diagnostic_report(report, format, path)` | `services::report::save` |
| `render_diagnostic_report(report, format)` | `services::report::render` |
| `load_diagnostic_report(path)` | `services::report::load` |
| `check_flutter_outdated(path)` | `services::outdated::check` |
| `save_log_export(path, content)` | 写入日志筛选结果 |
| `print_current_window()` | 调用系统打印面板（可存为 PDF） |

`save_diagnostic_report` 接收前端回传的报告对象，避免导出时重复执行耗时检测。
文件写入在 Rust 完成；前端只通过原生保存对话框拿路径。
环境与工具链命令都接受可选的 `force` 参数：页面首次加载复用 30 秒缓存，
用户点「重新检测」时传 `true` 绕过；报告生成复用缓存，重新生成时强制刷新。

### `services/`

真正的业务逻辑，全部可以在不启动 Tauri 的情况下用 `cargo test` 测试。

| 文件 | 职责 |
|---|---|
| `flutter.rs` | 查找 flutter（PATH → 常见目录 → 登录 shell）、执行 `--version --machine`、解析版本；包含 `doctor` 子模块 |
| `flutter/doctor.rs` | 执行并解析 `flutter doctor -v`，处理 verbose 前缀，保留可读报告 |
| `android.rs` | 读取 SDK 目录、JDK `release` 文件、licenses、AVD；不启动 adb / emulator |
| `ios.rs` | `xcode-select`、`xcodebuild`、`xcrun simctl`、`swift`、`pod`，带超时执行 |
| `system.rs` | `sw_vers`、`sysctl`、`uname`、`scutil`，读取 macOS / 芯片 / 内存 / 机型 |
| `toolchain.rs` | 并发检测 Python / Node.js / Git / Rust / Go / Java 的路径与版本 |
| `cache.rs` | 按类型分槽的 30 秒检测缓存，支持 `force` 绕过 |
| `project.rs` | 检查目录与 pubspec，组装平台 / Android / iOS / .metadata 信息；子模块负责解析 |
| `project/pubspec.rs` | pubspec / .metadata 的最小 YAML 子集解析 |
| `project/gradle.rs` | 从 `build.gradle(.kts)` 提取 applicationId / namespace / SDK 版本 |
| `project/ios_project.rs` | 从 Podfile 与 `project.pbxproj` 提取部署目标、Bundle ID |
| `build_log.rs` | 逐行统计错误 / 警告、合并重复行、识别构建结果 |
| `outdated.rs` | 显式执行 `flutter pub outdated --json` 并解析依赖更新 |
| `report.rs` | 聚合环境 / 项目 / 日志，计算摘要、渲染、载入与写盘 |
| `report/markdown.rs` | 报告 → Markdown 的纯渲染函数 |

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
| `toolchain.rs` | `ToolchainKind`、`ToolchainStatus`、`ToolchainSource`、`ToolchainInfo` |
| `outdated.rs` | `OutdatedStatus`、`OutdatedPackage`、`OutdatedReport` |
| `project.rs` | `ProjectStatus`、`ProjectDependency`、`ProjectPlatform`、`AndroidProjectConfig`、`IosProjectConfig`、`ProjectAnalysis` |
| `build_log.rs` | `BuildLogStatus`、`BuildOutcome`、`LogEntry`、`BuildLogAnalysis` |
| `report.rs` | `ReportOptions`、`ReportSummary`、`DiagnosticReport`、`ReportFormat`、`SavedReport` |

模型按检测对象拆分，不把 Android / iOS 字段塞进 `FlutterEnvironment`。
总览页的聚合在 UI 层完成，避免后端出现 God model。

### `utils/`

| 文件 | 职责 |
|---|---|
| `executable.rs` | PATH / 登录 shell / 可执行权限的共用查找逻辑 |
| `fs.rs` | `home_dir`、`is_executable`、`read_text_capped`（上限读取配置文件） |
| `process.rs` | `run_with_timeout`、输出上限、`ProcessOutput` / `ProcessError` |
| `time.rs` | 无依赖的 RFC 3339（UTC）格式化，供报告使用 |
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

### 常用工具链

- 检测对象：Python（python3）、Node.js（node）、Git、Rust（rustc）、Go、Java。
- PATH 拆分、登录 shell 执行和输出清洗与 Flutter / CocoaPods 共用
  `utils/executable.rs`，不再各写一套。
- 查找顺序：GUI PATH → 常见安装目录 → 登录 shell 的 `command -v`。
- 常见目录覆盖 pyenv / conda、nvm / fnm / Volta / asdf、Homebrew、SDKMAN、
  `/Library/Java/JavaVirtualMachines` 等版本管理器。
- `/usr/bin` 等系统目录会排到候选末尾：macOS 自带的 `python3` / `java` 可能是
  stub，开发者安装的版本优先；如果第一个候选执行失败，会继续尝试后续候选。
- 版本命令：`python3 --version`、`node --version`、`git --version`、
  `rustc --version`、`go version`、`java -version`（Java 输出在 stderr）。
- macOS 没有 JDK 时 `/usr/bin/java` 只是提示安装的占位程序，这种情况归为
  `NotFound` 而不是错误。
- 各工具用 scoped thread 并发检测；单个工具最多 5 秒，缺失时尝试登录 shell。
- 未安装或不可用的工具只影响工具链页展示，不改变诊断报告的总体结论。

### 检测缓存

- `services/cache.rs` 为每类检测维护一个 30 秒 TTL 的缓存槽。
- 总览、环境详情和诊断报告共用同一份缓存；页面首次加载用缓存，手动刷新传
  `force = true` 立即重测。
- 缓存只在内存中，退出应用即失效；失败结果也会短暂缓存，避免 TTL 内反复启动
  注定失败的外部命令。

### iOS / Xcode

- 开发者目录：`xcode-select -p`。
- Xcode：`/usr/bin/xcodebuild -version`；识别许可协议未接受的错误。
- Command Line Tools：检查 `/Library/Developer/CommandLineTools`。
- CocoaPods：PATH → RubyGems / rbenv / rvm / Homebrew → 登录 shell；执行 `pod --version`。
- 模拟器：`xcrun simctl list devices available --json`，只统计 iOS runtime。
- Swift：`swift --version`。

所有命令都有超时；检测失败不会 kill 用户的其他进程，也不会修改 Xcode 配置。

### Flutter 项目分析

- 输入是项目目录；没有 `pubspec.yaml` 直接判定为 `Error`，不会改目录内容。
- pubspec / .metadata 用最小 YAML 子集解析器（块映射、块序列、引号、行尾注释、
  `|` / `>` 块标量），不引入 YAML 依赖，也不执行 `flutter pub get`。
- 依赖来源分为 pub.dev / SDK / Git / 本地路径；资源与字体只统计声明，不读取文件内容。
- Android：优先 `android/app/build.gradle.kts`，否则 `build.gradle`；逐行匹配
  `applicationId` / `namespace` / `compileSdk` / `minSdk` / `targetSdk`，
  `flutter.compileSdkVersion` 这类表达式原样保留。
- iOS：Podfile 的 `platform :ios` 优先作为部署目标，否则读 `project.pbxproj`；
  Bundle ID 跳过 `Tests` target 和 `$(...)` 变量。
- 配置文件读取有上限（pubspec / Gradle 1 MiB，pbxproj 4 MiB）。
- 依赖列表支持前端筛选；资源声明解析成路径列表，可在页面展开查看。
- 「检查依赖更新」显式执行 `flutter pub outdated --json`，需要联网，不修改
  pubspec.yaml；结果包含当前 / 可升级 / 可解析 / 最新版本。

### 构建日志

- 以流式方式读取，最多分析 16 MiB / 200,000 行；超出时标记 `truncated`，不会把整个文件读进内存。
- 非 UTF-8 字节用替换字符处理；先剥离 ANSI 颜色码，再做分类和合并。
- 错误 / 警告是保守的启发式规则（`error` / `failed with` / `exception` / `warning` /
  `[!]` 等），不追求每种工具链的完整语法。
- 完全相同的行合并为一组，记录次数与首末行号；最多返回 50 组。
- 识别 `BUILD SUCCESSFUL` / `BUILD FAILED` / `** BUILD SUCCEEDED **` 等构建结果标记。
- 支持把日志文件拖进窗口分析；每条错误可复制内容或 `path:line`。
- 「复制筛选结果」写入剪贴板；「导出筛选结果」通过原生保存对话框写成文本文件。

### 诊断报告

- `generate` 依次执行本机配置、三套环境检测与常用工具链检测，可选追加项目 /
  日志分析；结果包含 `schemaVersion`（当前为 2）、UTC 生成时间和聚合摘要。
- 工具链完整列表在报告的 `toolchains` 字段和 Markdown「常用工具链」表格中。
- 报告页支持载入历史 JSON 做差异对比、复制 Markdown/JSON、打印或存为 PDF，
  并记录最近导出。
- `save` 接收前端回传的同一个报告对象，JSON 使用 pretty 格式，Markdown 由纯函数渲染。
- 导出路径必须由用户通过原生保存对话框选择；Rust 负责创建文件，前端不直接写盘。
- 导出成功后可在 Finder 中定位文件（opener 插件的 `revealItemInDir`）。
- 报告只包含本机检测结果，不包含源码内容或日志全文。

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

## 九、macOS 打包

`src-tauri/tauri.conf.json` 的 `bundle` 已配置为 macOS 桌面应用：

| 项 | 值 |
|---|---|
| `targets` | `app`、`dmg` |
| `category` | `DeveloperTool` |
| `macOS.minimumSystemVersion` | `12.0` |
| `dmg` | 660×400 窗口，应用与 Applications 快捷方式左右排列 |

打包命令：

```bash
npm run tauri build
```

产物在 `src-tauri/target/release/bundle/`：
`macos/DevScope.app` 与 `dmg/DevScope_0.1.0_aarch64.dmg`。

当前没有配置 Apple Developer 证书和公证（notarization）。本机构建会使用 ad-hoc
签名，可以自己运行；分发给其他用户前需要补签名与公证流程，这不属于 MVP 范围。

---

## 十、绝对不要手改的文件

| 路径 | 原因 |
|---|---|
| `node_modules/` | npm 自动管理 |
| `src-tauri/target/` | Cargo 编译产物 |
| `src-tauri/gen/` | Tauri 自动生成的权限 schema |
| `dist/` | 前端构建输出 |
| `package-lock.json` / `Cargo.lock` | 只让工具改，但要提交进 Git |
