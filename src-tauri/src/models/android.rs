//! Android 开发环境的数据契约。
//!
//! Android 的“环境”由 SDK 目录、JDK 和模拟器三部分组成，所以模型是组合式的，
//! 不把字段塞进 Flutter 模型。字段经 serde 转为 camelCase，与前端 TS 一一对应。

use serde::{Deserialize, Serialize};

/// 一次 Android 环境检测的业务结论。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AndroidStatus {
    /// SDK、JDK、platform、build-tools、licenses 以及 adb / emulator / sdkmanager 都可用。
    Ok,
    /// 找到了 SDK，但缺少某些组件；仍可能用真机开发。
    Warning,
    /// 常见位置和环境变量里都没有找到 Android SDK。
    NotFound,
    /// 找到了 SDK，但读取目录结构失败。
    Error,
}

/// Android SDK 是通过哪种方式发现的。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AndroidSdkSource {
    /// 来自 `ANDROID_HOME` 或 `ANDROID_SDK_ROOT`。
    Environment,
    /// 来自 macOS 上的常见安装目录。
    CommonLocation,
}

/// JDK 的发现方式，用于解释为什么选了这个 Java。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum JavaSource {
    /// 来自 `JAVA_HOME`。
    JavaHome,
    /// 来自 Android Studio 自带的 JBR。
    AndroidStudio,
    /// 来自系统 JDK、Homebrew 或 SDKMAN 等位置。
    System,
}

/// 一个可用的 JDK。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaRuntime {
    /// JDK 根目录（包含 `bin/java` 的目录）。
    pub path: String,
    /// 从 JDK 的 `release` 文件解析出的版本；读不到时为 None。
    pub version: Option<String>,
    /// 这个 JDK 是从哪一级查找中发现的。
    pub source: JavaSource,
}

/// 一次 Android 环境检测的完整结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AndroidEnvironment {
    /// 业务结论，而不是 IPC 层的成功/失败。
    pub status: AndroidStatus,
    /// Android SDK 根目录；未找到时为 None。
    pub sdk_path: Option<String>,
    /// SDK 目录的发现方式。
    pub sdk_source: Option<AndroidSdkSource>,
    /// `platforms/` 下的 platform 目录名，按版本从新到旧排列。
    pub platforms: Vec<String>,
    /// `build-tools/` 下的版本目录名，按版本从新到旧排列。
    pub build_tools: Vec<String>,
    /// `cmdline-tools/` 下的版本目录；存在 `latest` 时优先记录 `latest`。
    pub cmdline_tools_version: Option<String>,
    /// `sdkmanager` 的绝对路径。
    pub sdk_manager_path: Option<String>,
    /// `platform-tools/adb` 的绝对路径。
    pub adb_path: Option<String>,
    /// `emulator/emulator` 的绝对路径。
    pub emulator_path: Option<String>,
    /// `~/.android/avd` 下已配置的模拟器名称。
    pub avds: Vec<String>,
    /// 找到的 JDK；None 表示没有找到能执行的 Java。
    pub java: Option<JavaRuntime>,
    /// `licenses/` 下已接受的 license 文件数量。
    pub accepted_licenses: usize,
    /// 需要用户关注但不会中断检测的问题。
    pub issues: Vec<String>,
    /// 整体不可用时的原因；正常或只缺可选项时为 None。
    pub message: Option<String>,
    /// 本次探测过的 SDK 候选路径，便于排查为什么没找到。
    pub checked_paths: Vec<String>,
}
