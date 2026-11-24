//! 诊断报告的数据契约。
//!
//! 报告把本机配置、三个环境检测和可选的项目 / 日志分析聚合成一个可序列化对象。
//! 前端先生成报告做预览，再把同一个对象交回 Rust 写成 JSON / Markdown，
//! 避免导出时重复执行耗时的环境检测。

use serde::{Deserialize, Serialize};

use super::android::AndroidEnvironment;
use super::build_log::BuildLogAnalysis;
use super::environment::FlutterEnvironment;
use super::ios::IosEnvironment;
use super::project::ProjectAnalysis;
use super::system::SystemInfo;
use super::toolchain::ToolchainInfo;

/// 生成报告时可选的分析输入。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportOptions {
    /// 可选的 Flutter 项目目录。
    pub project_path: Option<String>,
    /// 可选的构建日志文件。
    pub log_path: Option<String>,
}

/// 报告的总体结论。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReportSummaryStatus {
    /// 环境和可选项都没有需要处理的问题。
    Ok,
    /// 有未安装、缺失或警告项。
    Warning,
    /// 有检测失败或项目 / 日志读取失败。
    Error,
}

/// 报告顶部的摘要，供页面和 Markdown 共用。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportSummary {
    pub status: ReportSummaryStatus,
    /// 需要关注的问题条目数量（环境 issues + 项目 issues + 日志错误 / 警告行）。
    pub issue_count: usize,
    /// 面向人的一行摘要，按「本机 → 环境 → 项目 → 日志」排列。
    pub highlights: Vec<String>,
}

/// 一次诊断的完整报告。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticReport {
    /// 报告结构版本；字段发生不兼容变化时递增。
    pub schema_version: u32,
    /// 生成工具及版本，例如 `DevScope 0.1.0`。
    pub generator: String,
    /// RFC 3339（UTC）生成时间。
    pub generated_at: String,
    /// 生成时间的 Unix 毫秒时间戳，前端按本地时区展示。
    pub generated_at_epoch_ms: u64,
    pub system: SystemInfo,
    pub flutter: FlutterEnvironment,
    pub android: AndroidEnvironment,
    pub ios: IosEnvironment,
    /// 常用工具链检测结果；缺失的工具不会让报告失败。
    #[serde(default)]
    pub toolchains: Vec<ToolchainInfo>,
    pub project: Option<ProjectAnalysis>,
    pub build_log: Option<BuildLogAnalysis>,
    pub summary: ReportSummary,
}

/// 导出格式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReportFormat {
    Json,
    Markdown,
}

/// 导出成功后的结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedReport {
    /// 实际写入的文件路径。
    pub path: String,
    pub format: ReportFormat,
    /// 写入的字节数。
    pub bytes: usize,
}
