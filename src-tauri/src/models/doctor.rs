//! `flutter doctor -v` 体检报告的数据契约。
//!
//! doctor 输出是面向人的文本，不是稳定 JSON。这里保留原始输出，同时提供
//! 轻量结构化的 section 摘要，供页面做状态卡片；解析失败时 UI 仍可展示原文。

use serde::Serialize;

/// 体检结论或单个检查项的结论。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DoctorStatus {
    /// 全部通过。
    Ok,
    /// 有需要关注的项，但不阻塞开发。
    Warning,
    /// 有失败项，或 doctor 本身没能正常产出报告。
    Error,
}

/// `flutter doctor -v` 中的一个检查项，例如 Flutter、Android toolchain、Xcode。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorSection {
    /// 检查项名称，来自 `[✓]` / `[!]` / `[✗]` 后面的标题。
    pub title: String,
    pub status: DoctorStatus,
    /// 适合卡片展示的一行摘要。warning/error 时优先取带 `!` 的提示行。
    pub summary: String,
    /// 该检查项下的明细行，已去掉 `•` / `!` 前缀。
    pub details: Vec<String>,
}

/// 一次 `flutter doctor -v` 体检的完整结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorReport {
    /// 整体结论：任一项 Error 则 Error，否则任一项 Warning 则 Warning。
    pub status: DoctorStatus,
    pub sections: Vec<DoctorSection>,
    /// 已剥离 Flutter verbose 前缀和内部日志的 doctor 报告文本，供「查看输出」使用。
    pub raw_output: String,
    /// doctor 整体失败时的原因；正常产出报告时为 None。
    pub message: Option<String>,
    pub truncated: bool,
}
