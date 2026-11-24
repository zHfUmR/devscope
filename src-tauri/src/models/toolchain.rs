//! 常用开发工具链的数据契约。
//!
//! 与 Flutter / Android / iOS 的完整环境检测不同，这里只回答「这个工具装了没有、
//! 是什么版本、装在哪」。字段经 serde 转为 camelCase，与前端 TS 一一对应。

use serde::{Deserialize, Serialize};

/// 支持的常用工具链。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ToolchainKind {
    Python,
    Node,
    Git,
    Rust,
    Go,
    Java,
}

/// 一次工具链检测的业务结论。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ToolchainStatus {
    /// 找到可执行文件并成功解析出版本。
    Ok,
    /// PATH、常见目录和登录 shell 中都没有找到。
    NotFound,
    /// 找到了，但执行失败、超时或输出无法解析。
    Error,
}

/// 可执行文件的发现方式，用于解释结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ToolchainSource {
    /// 来自 GUI 进程继承的 PATH。
    Path,
    /// 来自常见安装目录。
    CommonLocation,
    /// 通过用户登录 shell 的 `command -v` 解析得到。
    LoginShell,
}

/// 一个常用工具链的检测结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolchainInfo {
    /// 工具标识。
    pub kind: ToolchainKind,
    /// 展示名，例如 Python / Node.js。
    pub name: String,
    /// 实际查找的可执行文件名，例如 python3。
    pub binary: String,
    pub status: ToolchainStatus,
    /// 解析出的版本；未安装或失败时为 None。
    pub version: Option<String>,
    /// 可执行文件路径；未找到时为 None。
    pub executable_path: Option<String>,
    /// 路径的发现方式。
    pub source: Option<ToolchainSource>,
    /// 面向用户的失败原因。
    pub message: Option<String>,
    /// 本次探测过的候选路径，便于排查为什么没找到。
    pub checked_paths: Vec<String>,
}
