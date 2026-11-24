//! 构建日志分析的数据契约。
//!
//! 日志是自由的文本，这里不试图保留全文：只统计错误 / 警告，按内容合并重复项，
//! 并识别构建结果。字段经 serde 转为 camelCase，与前端 TS 一一对应。

use serde::{Deserialize, Serialize};

/// 一条日志记录的级别。分析只关心错误和警告，普通信息不进入结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LogSeverity {
    Error,
    Warning,
}

/// 从日志中识别出的构建结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BuildOutcome {
    /// 明确看到构建成功标记。
    Success,
    /// 明确看到构建失败标记。
    Failure,
    /// 日志中没有可识别的构建结果。
    #[default]
    Unknown,
}

/// 一次日志分析的业务结论。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BuildLogStatus {
    /// 没有发现错误或警告。
    Ok,
    /// 读取成功，但日志里有错误 / 警告，或日志被截断。
    Warning,
    /// 文件不存在、不是普通文件或读取失败。
    Error,
}

/// 一组内容相同的日志记录。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub severity: LogSeverity,
    /// 去掉 ANSI 控制符并 trim 后的整行文本。
    pub message: String,
    /// 这行内容出现的次数。
    pub count: usize,
    /// 第一次出现的行号（从 1 开始）。
    pub first_line: usize,
    /// 最后一次出现的行号。
    pub last_line: usize,
}

/// 一次构建日志分析的完整结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildLogAnalysis {
    pub status: BuildLogStatus,
    /// 用户选择的日志文件路径。
    pub path: String,
    /// 文件名，便于页面标题展示。
    pub file_name: String,
    /// 文件总字节数。
    pub file_size_bytes: u64,
    /// 实际读取并分析的字节数。
    pub analyzed_bytes: u64,
    /// 已分析的行数。
    pub total_lines: usize,
    /// 是否因为文件过大而只分析了前一部分。
    pub truncated: bool,
    /// 错误行数量（按行计，不是按分组计）。
    pub error_count: usize,
    /// 警告行数量。
    pub warning_count: usize,
    /// 合并后的错误 / 警告分组，按严重程度和出现次数排序。
    pub entries: Vec<LogEntry>,
    /// 因结果过多而未展示的分组数量。
    pub omitted_entries: usize,
    /// 日志中识别出的构建结果。
    pub build_outcome: BuildOutcome,
    /// 文件为空、读取失败或日志被截断时的说明。
    pub message: Option<String>,
}
