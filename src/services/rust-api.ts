/**
 * 所有前端 → Rust 的 IPC 调用都集中在这里。
 *
 * 组件里不要直接 import invoke；这样命令名、参数和返回类型只有一处定义，
 * 改 Rust 契约时也只需要在同一处更新。
 */

import { invoke } from "@tauri-apps/api/core";
import type {
  AndroidEnvironment,
  DoctorReport,
  FlutterEnvironment,
  IosEnvironment,
  SystemInfo,
} from "../types";

/**
 * 检测本机 Flutter 环境。
 *
 * 业务失败（未安装、超时、输出异常）不会 reject，而是通过返回值的
 * `status` / `message` 表达；只有 IPC 或命令执行机制本身出错时才会 reject。
 */
export async function detectFlutterEnvironment(): Promise<FlutterEnvironment> {
  try {
    return await invoke<FlutterEnvironment>("detect_flutter_environment");
  } catch (error) {
    throw new Error(toErrorMessage(error));
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
    throw new Error(toErrorMessage(error));
  }
}

/**
 * 检测本机 Android SDK、JDK 和模拟器。
 *
 * 业务结果（未安装、缺组件）通过返回值的 `status` / `issues` 表达；
 * 只有 IPC 或命令执行机制本身出错时才会 reject。
 */
export async function detectAndroidEnvironment(): Promise<AndroidEnvironment> {
  try {
    return await invoke<AndroidEnvironment>("detect_android_environment");
  } catch (error) {
    throw new Error(toErrorMessage(error));
  }
}

/**
 * 检测本机 Xcode、Command Line Tools、CocoaPods 和 iOS 模拟器。
 *
 * 这个命令会执行 xcodebuild / xcrun / pod，通常需要数秒；页面应在调用期间
 * 展示独立的 loading 状态。
 */
export async function detectIosEnvironment(): Promise<IosEnvironment> {
  try {
    return await invoke<IosEnvironment>("detect_ios_environment");
  } catch (error) {
    throw new Error(toErrorMessage(error));
  }
}

/**
 * 读取本机配置：macOS、芯片、内存、核心数和机型。
 *
 * 这个命令只读系统信息，通常很快；单个字段读取失败时为 null，不会 reject。
 */
export async function detectSystemInfo(): Promise<SystemInfo> {
  try {
    return await invoke<SystemInfo>("detect_system_info");
  } catch (error) {
    throw new Error(toErrorMessage(error));
  }
}

/**
 * 把 Tauri 的未知错误值收敛成可读字符串。
 *
 * Rust 命令返回 `Err(String)` 时，invoke 通常会 reject 一个字符串；这里同时
 * 兼容 Error 和其他未知值，避免 UI 层到处写类型判断。
 */
function toErrorMessage(error: unknown): string {
  if (error instanceof Error && error.message.trim()) {
    return error.message;
  }
  if (typeof error === "string" && error.trim()) {
    return error;
  }

  return "调用 Rust 命令失败，请查看应用日志。";
}
