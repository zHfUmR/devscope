//! 开发环境检测的数据契约。字段经 serde 转为 camelCase，与前端 TS 一一对应。

use serde::Serialize;

/// 一次 Flutter 环境检测的业务结论。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FlutterStatus {
    /// 找到 flutter 且版本信息解析成功。
    Ok,
    /// PATH、常见安装路径和登录 shell 中都没有找到 flutter。
    NotFound,
    /// 找到了 flutter，但执行失败、超时或输出无法解析。
    Error,
}

/// flutter 可执行文件的发现方式，用于向用户解释检测结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FlutterExecutableSource {
    /// 来自 GUI 进程继承的 PATH。
    Path,
    /// 来自已知的常见安装目录。
    CommonLocation,
    /// 通过用户登录 shell 的 `command -v flutter` 解析得到。
    LoginShell,
}

/// `flutter --version --machine` 中与版本相关的字段。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlutterVersionInfo {
    /// Flutter 框架版本，例如 3.35.3。
    pub framework_version: String,
    /// 发布渠道，例如 stable / beta / main。
    pub channel: Option<String>,
    /// Flutter 自带 Dart SDK 的版本。
    pub dart_sdk_version: Option<String>,
    /// Engine 的 Git revision。
    pub engine_revision: Option<String>,
    /// Flutter Framework 的 Git revision。
    pub framework_revision: Option<String>,
    /// Framework 提交时间，保留 Flutter 原始格式。
    pub framework_commit_date: Option<String>,
    /// Flutter DevTools 版本。
    pub dev_tools_version: Option<String>,
}

/// 一次 Flutter 环境检测的完整结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlutterEnvironment {
    /// 业务结论，而不是 IPC 层的成功/失败。
    pub status: FlutterStatus,
    /// 找到的 flutter 可执行文件路径；未找到或执行失败时仍可能有值。
    pub executable_path: Option<String>,
    /// 该路径是通过哪种方式发现的。
    pub executable_source: Option<FlutterExecutableSource>,
    /// 解析出的版本信息；只有检测成功时才有值。
    pub version: Option<FlutterVersionInfo>,
    /// 面向用户的失败原因；成功时为 None。
    pub message: Option<String>,
    /// 本次探测过的候选路径，便于用户排查为什么没找到。
    pub checked_paths: Vec<String>,
}
