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
 * 检测本机 Flutter 环境。
 *
 * 业务失败（未安装、超时、输出异常）不会 reject，而是通过返回值的
 * `status` / `message` 表达；只有 IPC 或命令执行机制本身出错时才会 reject。
 */
export async function detectFlutterEnvironment(force = false): Promise<FlutterEnvironment> {
  try {
    return await invoke<FlutterEnvironment>("detect_flutter_environment", { force });
  } catch (error) {
    throw new Error(toUserMessage(error));
  }
}

/**
 * 运行 `flutter doctor -v` 体检。
 *
 * 这是按需触发的慢操作，可能耗时数十秒。doctor 报告内部的 warning/error
 * 通过返回值表达，只有 IPC 或任务本身失败时才 reject。
 */
export async function runFlutterDoctor(): Promise<DoctorReport> {
  try {
    return await invoke<DoctorReport>("run_flutter_doctor");
  } catch (error) {
    throw new Error(toUserMessage(error));
  }
}

/**
 * 检测本机 Android SDK、JDK 和模拟器。
 *
 * 业务结果（未安装、缺组件）通过返回值的 `status` / `issues` 表达；
 * 只有 IPC 或命令执行机制本身出错时才会 reject。
 */
export async function detectAndroidEnvironment(force = false): Promise<AndroidEnvironment> {
  try {
    return await invoke<AndroidEnvironment>("detect_android_environment", { force });
  } catch (error) {
    throw new Error(toUserMessage(error));
  }
}

/**
 * 检测本机 Xcode、Command Line Tools、CocoaPods 和 iOS 模拟器。
 *
 * 这个命令会执行 xcodebuild / xcrun / pod，通常需要数秒；页面应在调用期间
 * 展示独立的 loading 状态。
 */
export async function detectIosEnvironment(force = false): Promise<IosEnvironment> {
  try {
    return await invoke<IosEnvironment>("detect_ios_environment", { force });
  } catch (error) {
    throw new Error(toUserMessage(error));
  }
}

/**
 * 读取本机配置：macOS、芯片、内存、核心数和机型。
 *
 * 这个命令只读系统信息，通常很快；单个字段读取失败时为 null，不会 reject。
 */
export async function detectSystemInfo(force = false): Promise<SystemInfo> {
  try {
    return await invoke<SystemInfo>("detect_system_info", { force });
  } catch (error) {
    throw new Error(toUserMessage(error));
  }
}

/**
 * 检测常用工具链（Python / Node.js / Git / Rust / Go / Java）。
 *
 * 每个工具独立检测，未安装或执行失败不会 reject，而是通过返回项的
 * `status` / `message` 表达；只有 IPC 或任务机制失败时才 reject。
 */
export async function detectToolchains(force = false): Promise<ToolchainInfo[]> {
  try {
    return await invoke<ToolchainInfo[]>("detect_toolchains", { force });
  } catch (error) {
    throw new Error(toUserMessage(error, "检测常用工具链失败，请查看应用日志。"));
  }
}

/**
 * 检查 Flutter 项目依赖更新。
 *
 * 会执行 `flutter pub outdated --json` 并访问 pub.dev，属于显式联网操作；
 * 只在用户点击按钮时调用。
 */
export async function checkFlutterOutdated(path: string): Promise<OutdatedReport> {
  try {
    return await invoke<OutdatedReport>("check_flutter_outdated", { path });
  } catch (error) {
    throw new Error(toUserMessage(error, "检查依赖更新失败，请查看应用日志。"));
  }
}

/**
 * 分析一个 Flutter 项目目录。
 *
 * 业务结果（不是 Flutter 项目、缺少配置）通过返回值的 `status` / `issues`
 * 表达；只有 IPC 或任务本身失败时才 reject。
 */
export async function analyzeFlutterProject(path: string): Promise<ProjectAnalysis> {
  try {
    return await invoke<ProjectAnalysis>("analyze_flutter_project", { path });
  } catch (error) {
    throw new Error(toUserMessage(error));
  }
}

/**
 * 分析一份构建日志文件。
 *
 * 日志里的错误 / 警告通过返回值的 `entries` 表达；文件不存在或读取失败
 * 也会收敛到 `status: "error"` 和 `message`，不会 reject。
 */
export async function analyzeBuildLog(path: string): Promise<BuildLogAnalysis> {
  try {
    return await invoke<BuildLogAnalysis>("analyze_build_log", { path });
  } catch (error) {
    throw new Error(toUserMessage(error));
  }
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
export async function generateDiagnosticReport(
  options: DiagnosticReportOptions,
  force = false,
): Promise<DiagnosticReport> {
  try {
    return await invoke<DiagnosticReport>("generate_diagnostic_report", {
      projectPath: options.projectPath,
      logPath: options.logPath,
      force,
    });
  } catch (error) {
    throw new Error(toUserMessage(error, "生成诊断报告失败，请查看应用日志。"));
  }
}

/**
 * 把已生成的报告导出为 JSON / Markdown。
 *
 * `report` 直接回传 Rust，避免为了导出重新执行环境检测；文件写入在 Rust
 * 侧完成，前端不直接操作文件系统。
 */
export async function saveDiagnosticReport(
  report: DiagnosticReport,
  format: ReportFormat,
  path: string,
): Promise<SavedReport> {
  try {
    return await invoke<SavedReport>("save_diagnostic_report", {
      report,
      format,
      path,
    });
  } catch (error) {
    throw new Error(toUserMessage(error, "导出诊断报告失败，请查看应用日志。"));
  }
}

/**
 * 把报告渲染成 JSON / Markdown 字符串，用于复制到剪贴板。
 *
 * 渲染在 Rust 侧完成，前端不重复实现 Markdown 格式。
 */
export async function renderDiagnosticReport(
  report: DiagnosticReport,
  format: ReportFormat,
): Promise<string> {
  try {
    return await invoke<string>("render_diagnostic_report", { report, format });
  } catch (error) {
    throw new Error(toUserMessage(error, "渲染诊断报告失败，请查看应用日志。"));
  }
}

/** 读取一份之前导出的 JSON 报告，用于对比。 */
export async function loadDiagnosticReport(path: string): Promise<DiagnosticReport> {
  try {
    return await invoke<DiagnosticReport>("load_diagnostic_report", { path });
  } catch (error) {
    throw new Error(toUserMessage(error, "载入历史报告失败，请确认文件是 DevScope JSON 报告。"));
  }
}

/** 把日志筛选结果写入用户选择的文本文件。 */
export async function saveLogExport(path: string, content: string): Promise<void> {
  try {
    await invoke("save_log_export", { path, content });
  } catch (error) {
    throw new Error(toUserMessage(error, "导出日志结果失败，请查看应用日志。"));
  }
}

/** 打开系统打印面板，可另存为 PDF。 */
export async function printCurrentWindow(): Promise<void> {
  try {
    await invoke("print_current_window");
  } catch (error) {
    throw new Error(toUserMessage(error, "调用打印面板失败。"));
  }
}
