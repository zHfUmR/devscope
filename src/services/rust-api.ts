/**
 * 所有前端 → Rust 的 IPC 调用都集中在这里。
 *
 * 组件里不要直接 import invoke；这样命令名、参数和返回类型只有一处定义，
 * 改 Rust 契约时也只需要在同一处更新。
 */

import { invoke } from "@tauri-apps/api/core";
import type {
  AndroidEnvironment,
  BuildLogAnalysis,
  DiagnosticReport,
  DoctorReport,
  FlutterEnvironment,
  IosEnvironment,
  OutdatedReport,
  ProjectAnalysis,
  ReportFormat,
  SavedReport,
  SystemInfo,
  ToolchainInfo,
} from "../types";
import { toUserMessage } from "./errors";

/**
 * 统一的 IPC 调用入口。
 *
 * 命令名和参数对象的 key 必须与 Rust 侧逐字一致。任何失败（Rust 命令返回的
 * `Err(String)`、或 IPC 机制本身出错）都收敛成一条用户可读的 Error 抛出，
 * 这样每个导出函数都只是一行参数转发。
 */
async function call<T>(
  command: string,
  args?: Record<string, unknown>,
  fallback?: string,
): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    throw new Error(toUserMessage(error, fallback));
  }
}

/**
 * 检测本机 Flutter 环境。
 *
 * 业务失败（未安装、超时、输出异常）不会 reject，而是通过返回值的
 * `status` / `message` 表达；只有 IPC 或命令执行机制本身出错时才会 reject。
 */
export function detectFlutterEnvironment(force = false): Promise<FlutterEnvironment> {
  return call("detect_flutter_environment", { force });
}

/**
 * 运行 `flutter doctor -v` 体检。
 *
 * 这是按需触发的慢操作，可能耗时数十秒。doctor 报告内部的 warning/error
 * 通过返回值表达，只有 IPC 或任务本身失败时才 reject。
 */
export function runFlutterDoctor(): Promise<DoctorReport> {
  return call("run_flutter_doctor");
}

/**
 * 检测本机 Android SDK、JDK 和模拟器。
 *
 * 业务结果（未安装、缺组件）通过返回值的 `status` / `issues` 表达；
 * 只有 IPC 或命令执行机制本身出错时才会 reject。
 */
export function detectAndroidEnvironment(force = false): Promise<AndroidEnvironment> {
  return call("detect_android_environment", { force });
}

/**
 * 检测本机 Xcode、Command Line Tools、CocoaPods 和 iOS 模拟器。
 *
 * 这个命令会执行 xcodebuild / xcrun / pod，通常需要数秒；页面应在调用期间
 * 展示独立的 loading 状态。
 */
export function detectIosEnvironment(force = false): Promise<IosEnvironment> {
  return call("detect_ios_environment", { force });
}

/**
 * 读取本机配置：macOS、芯片、内存、核心数和机型。
 *
 * 这个命令只读系统信息，通常很快；单个字段读取失败时为 null，不会 reject。
 */
export function detectSystemInfo(force = false): Promise<SystemInfo> {
  return call("detect_system_info", { force });
}

/**
 * 检测常用工具链（Python / Node.js / Git / Rust / Go / Java）。
 *
 * 每个工具独立检测，未安装或执行失败不会 reject，而是通过返回项的
 * `status` / `message` 表达；只有 IPC 或任务机制失败时才 reject。
 */
export function detectToolchains(force = false): Promise<ToolchainInfo[]> {
  return call("detect_toolchains", { force }, "检测常用工具链失败，请查看应用日志。");
}

/**
 * 检查 Flutter 项目依赖更新。
 *
 * 会执行 `flutter pub outdated --json` 并访问 pub.dev，属于显式联网操作；
 * 只在用户点击按钮时调用。
 */
export function checkFlutterOutdated(path: string): Promise<OutdatedReport> {
  return call("check_flutter_outdated", { path }, "检查依赖更新失败，请查看应用日志。");
}

/**
 * 分析一个 Flutter 项目目录。
 *
 * 业务结果（不是 Flutter 项目、缺少配置）通过返回值的 `status` / `issues`
 * 表达；只有 IPC 或任务本身失败时才 reject。
 */
export function analyzeFlutterProject(path: string): Promise<ProjectAnalysis> {
  return call("analyze_flutter_project", { path });
}

/**
 * 分析一份构建日志文件。
 *
 * 日志里的错误 / 警告通过返回值的 `entries` 表达；文件不存在或读取失败
 * 也会收敛到 `status: "error"` 和 `message`，不会 reject。
 */
export function analyzeBuildLog(path: string): Promise<BuildLogAnalysis> {
  return call("analyze_build_log", { path });
}

/** 生成诊断报告时可选的项目与日志输入。 */
export interface DiagnosticReportOptions {
  projectPath: string | null;
  logPath: string | null;
}

/**
 * 生成一次完整诊断报告。
 *
 * 会执行 Flutter / Android / iOS 三套检测，可能耗时数秒到数十秒；
 * 页面需要独立的 loading 状态。业务结论在 `summary` 中。
 */
export function generateDiagnosticReport(
  options: DiagnosticReportOptions,
  force = false,
): Promise<DiagnosticReport> {
  return call(
    "generate_diagnostic_report",
    {
      projectPath: options.projectPath,
      logPath: options.logPath,
      force,
    },
    "生成诊断报告失败，请查看应用日志。",
  );
}

/**
 * 把已生成的报告导出为 JSON / Markdown。
 *
 * `report` 直接回传 Rust，避免为了导出重新执行环境检测；文件写入在 Rust
 * 侧完成，前端不直接操作文件系统。
 */
export function saveDiagnosticReport(
  report: DiagnosticReport,
  format: ReportFormat,
  path: string,
): Promise<SavedReport> {
  return call(
    "save_diagnostic_report",
    { report, format, path },
    "导出诊断报告失败，请查看应用日志。",
  );
}

/**
 * 把报告渲染成 JSON / Markdown 字符串，用于复制到剪贴板。
 *
 * 渲染在 Rust 侧完成，前端不重复实现 Markdown 格式。
 */
export function renderDiagnosticReport(
  report: DiagnosticReport,
  format: ReportFormat,
): Promise<string> {
  return call("render_diagnostic_report", { report, format }, "渲染诊断报告失败，请查看应用日志。");
}

/** 读取一份之前导出的 JSON 报告，用于对比。 */
export function loadDiagnosticReport(path: string): Promise<DiagnosticReport> {
  return call(
    "load_diagnostic_report",
    { path },
    "载入历史报告失败，请确认文件是 DevScope JSON 报告。",
  );
}

/** 把日志筛选结果写入用户选择的文本文件。 */
export function saveLogExport(path: string, content: string): Promise<void> {
  return call("save_log_export", { path, content }, "导出日志结果失败，请查看应用日志。");
}

/** 打开系统打印面板，可另存为 PDF。 */
export function printCurrentWindow(): Promise<void> {
  return call("print_current_window", undefined, "调用打印面板失败。");
}
