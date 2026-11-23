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
