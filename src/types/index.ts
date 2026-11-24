/**
 * 与 Rust `src-tauri/src/models/` 下的模型一一对应的 IPC 数据契约。
 *
 * 覆盖 environment.rs（Flutter）、doctor.rs、android.rs、ios.rs、system.rs。
 * Rust 侧使用 `#[serde(rename_all = "camelCase")]`，所以这里全部是 camelCase。
 * `Option<T>` 序列化为 `T | null`，`Vec<T>` 序列化为 `T[]`。
 */

/** 一次 Flutter 环境检测的业务结论。 */
export type FlutterStatus = "ok" | "notFound" | "error";

/** flutter 可执行文件的发现方式。 */
export type FlutterExecutableSource = "path" | "commonLocation" | "loginShell";

/** `flutter --version --machine` 中与版本相关的字段。 */
export interface FlutterVersionInfo {
  frameworkVersion: string;
  channel: string | null;
  dartSdkVersion: string | null;
  engineRevision: string | null;
  frameworkRevision: string | null;
  frameworkCommitDate: string | null;
  devToolsVersion: string | null;
}

/** 一次 Flutter 环境检测的完整结果。 */
export interface FlutterEnvironment {
  status: FlutterStatus;
  executablePath: string | null;
  executableSource: FlutterExecutableSource | null;
  version: FlutterVersionInfo | null;
  /** 面向用户的失败原因；成功时为 null。 */
  message: string | null;
  /** 本次探测过的候选路径，可用于未找到或失败时的排查展示。 */
  checkedPaths: string[];
}

/** `flutter doctor -v` 中单个检查项的结论。 */
export type DoctorStatus = "ok" | "warning" | "error";

/** doctor 输出中的一个检查项。 */
export interface DoctorSection {
  title: string;
  status: DoctorStatus;
  /** 适合卡片展示的一行摘要。 */
  summary: string;
  /** 已去掉 `•` / `!` 前缀的明细行。 */
  details: string[];
}

/** 一次 `flutter doctor -v` 体检的完整结果。 */
export interface DoctorReport {
  status: DoctorStatus;
  sections: DoctorSection[];
  /** 已剥离 verbose 前缀和内部日志的 doctor 报告文本。 */
  rawOutput: string;
  /** doctor 整体失败时的原因。 */
  message: string | null;
  truncated: boolean;
}

/** 一次 Android 环境检测的业务结论。 */
export type AndroidStatus = "ok" | "warning" | "notFound" | "error";

/** Android SDK 是通过哪种方式发现的。 */
export type AndroidSdkSource = "environment" | "commonLocation";

/** JDK 的发现方式。 */
export type JavaSource = "javaHome" | "androidStudio" | "system";

/** 一个可用的 JDK。 */
export interface JavaRuntime {
  /** JDK 根目录。 */
  path: string;
  version: string | null;
  source: JavaSource;
}

/** 一次 Android 环境检测的完整结果。 */
export interface AndroidEnvironment {
  status: AndroidStatus;
  sdkPath: string | null;
  sdkSource: AndroidSdkSource | null;
  /** `platforms/` 下的目录名，按版本从新到旧。 */
  platforms: string[];
  /** `build-tools/` 下的版本，按版本从新到旧。 */
  buildTools: string[];
  cmdlineToolsVersion: string | null;
  sdkManagerPath: string | null;
  adbPath: string | null;
  emulatorPath: string | null;
  /** `~/.android/avd` 下已配置的模拟器名称。 */
  avds: string[];
  java: JavaRuntime | null;
  /** `licenses/` 下已接受的 license 文件数量。 */
  acceptedLicenses: number;
  issues: string[];
  message: string | null;
  checkedPaths: string[];
}

/** 一次 iOS 环境检测的业务结论。 */
export type IosStatus = "ok" | "warning" | "notFound" | "error";

/** CocoaPods 可执行文件的发现方式。 */
export type CocoaPodsSource = "path" | "commonLocation" | "loginShell";

/** 一个可用的 CocoaPods 安装。 */
export interface CocoaPodsInfo {
  path: string;
  version: string | null;
  source: CocoaPodsSource;
}

/** iOS 模拟器的汇总信息。 */
export interface SimulatorInfo {
  availableCount: number;
  /** 已安装的 iOS runtime，例如 `iOS 27.0`。 */
  runtimes: string[];
}

/** 一次 iOS / Xcode 环境检测的完整结果。 */
export interface IosEnvironment {
  status: IosStatus;
  developerDir: string | null;
  xcodePath: string | null;
  xcodeVersion: string | null;
  xcodeBuild: string | null;
  commandLineToolsPath: string | null;
  licenseAccepted: boolean | null;
  cocoapods: CocoaPodsInfo | null;
  simulators: SimulatorInfo | null;
  swiftVersion: string | null;
  issues: string[];
  message: string | null;
  checkedPaths: string[];
}

/** 一台 macOS 主机的基础配置。 */
export interface SystemInfo {
  osName: string | null;
  osVersion: string | null;
  osBuild: string | null;
  architecture: string | null;
  model: string | null;
  chip: string | null;
  cpuCores: number | null;
  /** 物理内存字节数，前端负责格式化。 */
  memoryBytes: number | null;
  hostname: string | null;
}

/** 一次项目分析的业务结论。 */
export type ProjectStatus = "ok" | "warning" | "error";

/** 依赖在 pubspec.yaml 中属于哪一组。 */
export type DependencyKind = "regular" | "dev" | "override";

/** 依赖的来源类型。 */
export type DependencySource = "hosted" | "sdk" | "git" | "path" | "unknown";

/** pubspec.yaml 中声明的一个依赖。 */
export interface ProjectDependency {
  name: string;
  /** 版本约束或来源描述；`any` 等空约束为 null。 */
  constraint: string | null;
  kind: DependencyKind;
  source: DependencySource;
}

/** 项目启用的目标平台。 */
export type PlatformKind = "android" | "ios" | "web" | "macos" | "linux" | "windows";

/** 一个已存在的平台目录。 */
export interface ProjectPlatform {
  platform: PlatformKind;
  /** 相对项目根目录的目录名，例如 `android`。 */
  directory: string;
}

/** Android 模块的关键 Gradle 配置。 */
export interface AndroidProjectConfig {
  /** 读取的 Gradle 文件，相对项目根目录。 */
  gradleFile: string;
  /** 是否为 Kotlin DSL（`build.gradle.kts`）。 */
  kotlinDsl: boolean;
  applicationId: string | null;
  namespace: string | null;
  compileSdk: string | null;
  minSdk: string | null;
  targetSdk: string | null;
}

/** iOS 模块的关键配置。 */
export interface IosProjectConfig {
  deploymentTarget: string | null;
  bundleIdentifier: string | null;
  podfilePresent: boolean;
  workspacePresent: boolean;
  infoPlistPresent: boolean;
}

/** `.metadata` 文件中的 Flutter 工具信息。 */
export interface ProjectMetadata {
  channel: string | null;
  revision: string | null;
  projectType: string | null;
}

/** 一次 Flutter 项目分析的完整结果。 */
export interface ProjectAnalysis {
  status: ProjectStatus;
  rootPath: string;
  name: string | null;
  description: string | null;
  version: string | null;
  publishTo: string | null;
  dartSdkConstraint: string | null;
  flutterConstraint: string | null;
  dependencies: ProjectDependency[];
  devDependencies: ProjectDependency[];
  dependencyOverrides: ProjectDependency[];
  platforms: ProjectPlatform[];
  usesMaterialDesign: boolean;
  assetCount: number;
  assets: string[];
  fontFamilies: string[];
  android: AndroidProjectConfig | null;
  ios: IosProjectConfig | null;
  metadata: ProjectMetadata | null;
  issues: string[];
  message: string | null;
}

/** 一条日志记录的级别。 */
export type LogSeverity = "error" | "warning";

/** 从日志中识别出的构建结果。 */
export type BuildOutcome = "success" | "failure" | "unknown";

/** 一次日志分析的业务结论。 */
export type BuildLogStatus = "ok" | "warning" | "error";

/** 一组内容相同的日志记录。 */
export interface LogEntry {
  severity: LogSeverity;
  message: string;
  count: number;
  firstLine: number;
  lastLine: number;
}

/** 一次构建日志分析的完整结果。 */
export interface BuildLogAnalysis {
  status: BuildLogStatus;
  path: string;
  fileName: string;
  fileSizeBytes: number;
  analyzedBytes: number;
  totalLines: number;
  truncated: boolean;
  errorCount: number;
  warningCount: number;
  entries: LogEntry[];
  omittedEntries: number;
  buildOutcome: BuildOutcome;
  message: string | null;
}

/** 报告的总体结论。 */
export type ReportSummaryStatus = "ok" | "warning" | "error";

/** 报告顶部的摘要。 */
export interface ReportSummary {
  status: ReportSummaryStatus;
  issueCount: number;
  highlights: string[];
}

/** 一次诊断的完整报告。 */
export interface DiagnosticReport {
  schemaVersion: number;
  generator: string;
  /** RFC 3339（UTC）生成时间。 */
  generatedAt: string;
  /** 生成时间的 Unix 毫秒时间戳。 */
  generatedAtEpochMs: number;
  system: SystemInfo;
  flutter: FlutterEnvironment;
  android: AndroidEnvironment;
  ios: IosEnvironment;
  toolchains: ToolchainInfo[];
  project: ProjectAnalysis | null;
  buildLog: BuildLogAnalysis | null;
  summary: ReportSummary;
}

/** 导出格式。 */
export type ReportFormat = "json" | "markdown";

/** 导出成功后的结果。 */
export interface SavedReport {
  path: string;
  format: ReportFormat;
  bytes: number;
}

/** 支持的常用工具链。 */
export type ToolchainKind = "python" | "node" | "git" | "rust" | "go" | "java";

/** 一次工具链检测的业务结论。 */
export type ToolchainStatus = "ok" | "notFound" | "error";

/** 可执行文件的发现方式。 */
export type ToolchainSource = "path" | "commonLocation" | "loginShell";

/** 一个常用工具链的检测结果。 */
export interface ToolchainInfo {
  kind: ToolchainKind;
  /** 展示名，例如 Python / Node.js。 */
  name: string;
  /** 实际查找的可执行文件名，例如 python3。 */
  binary: string;
  status: ToolchainStatus;
  version: string | null;
  executablePath: string | null;
  source: ToolchainSource | null;
  /** 面向用户的失败原因。 */
  message: string | null;
  /** 本次探测过的候选路径。 */
  checkedPaths: string[];
}

/** 依赖更新检查的结论。 */
export type OutdatedStatus = "ok" | "warning" | "error";

/** 单个依赖的版本情况。 */
export interface OutdatedPackage {
  name: string;
  current: string | null;
  upgradable: string | null;
  resolvable: string | null;
  latest: string | null;
}

/** 一次依赖更新检查的结果。 */
export interface OutdatedReport {
  status: OutdatedStatus;
  packages: OutdatedPackage[];
  message: string | null;
}
