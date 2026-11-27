//! 项目分析、构建日志分析与诊断报告相关的 Tauri 命令。
//!
//! 和 `environment` 命令一样，这一层只做参数转发：把可能耗时的文件读取、
//! 进程执行丢到 blocking 线程池，并把机制性失败转成前端可读的字符串。
//! 业务失败（不是 Flutter 项目、日志读不到等）已经收敛在返回模型的 status
//! / message 里。

use std::fs;
use std::path::{Path, PathBuf};

use super::run_blocking;
use crate::models::build_log::BuildLogAnalysis;
use crate::models::outdated::OutdatedReport;
use crate::models::project::ProjectAnalysis;
use crate::models::report::{DiagnosticReport, ReportFormat, ReportOptions, SavedReport};
use crate::services::cache::DetectionCache;
use crate::services::{build_log, outdated, project, report};

/// 分析一个 Flutter 项目目录。
#[tauri::command]
pub async fn analyze_flutter_project(path: String) -> Result<ProjectAnalysis, String> {
    run_blocking("项目分析", move || project::analyze(Path::new(&path))).await
}

/// 分析一份构建日志文件。
#[tauri::command]
pub async fn analyze_build_log(path: String) -> Result<BuildLogAnalysis, String> {
    run_blocking("日志分析", move || build_log::analyze(Path::new(&path))).await
}

/// 生成一次完整诊断报告。
///
/// 会执行 Flutter / Android / iOS 三套检测，可能耗时数秒到数十秒；项目与日志
/// 是可选项，未提供时对应字段为 null。
#[tauri::command]
pub async fn generate_diagnostic_report(
    project_path: Option<String>,
    log_path: Option<String>,
    force: Option<bool>,
    cache: tauri::State<'_, DetectionCache>,
) -> Result<DiagnosticReport, String> {
    let options = ReportOptions { project_path, log_path };
    let cache = cache.inner().clone();
    let force = force.unwrap_or(false);

    run_blocking("诊断报告生成", move || report::generate(&options, &cache, force)).await
}

/// 把前端已生成的报告写成 JSON / Markdown 文件。
///
/// 报告的生成与导出分离，导出时不会重新执行环境检测，确保文件内容与预览一致。
#[tauri::command]
pub async fn save_diagnostic_report(
    report: DiagnosticReport,
    format: ReportFormat,
    path: String,
) -> Result<SavedReport, String> {
    let path = PathBuf::from(path);

    run_blocking("诊断报告导出", move || {
        report::save(&report, format, &path).map_err(|error| error.to_string())
    })
    .await?
}

/// 检查项目依赖是否可以更新。
///
/// 会执行 `flutter pub outdated --json` 并访问 pub.dev，属于显式的联网操作，
/// 只在用户点击按钮时调用。业务失败（找不到 pubspec / Flutter、命令失败）
/// 收敛在 `OutdatedReport.status` / `message` 里。
#[tauri::command]
pub async fn check_flutter_outdated(path: String) -> Result<OutdatedReport, String> {
    run_blocking("依赖更新检查", move || outdated::check(Path::new(&path))).await
}

/// 把报告渲染成字符串，供复制到剪贴板。
#[tauri::command]
pub async fn render_diagnostic_report(report: DiagnosticReport, format: ReportFormat) -> Result<String, String> {
    run_blocking("诊断报告渲染", move || report::render(&report, format).map_err(|error| error.to_string())).await?
}

/// 读取一份之前导出的 JSON 报告，用于版本对比。
#[tauri::command]
pub async fn load_diagnostic_report(path: String) -> Result<DiagnosticReport, String> {
    run_blocking("诊断报告载入", move || report::load(Path::new(&path)).map_err(|error| error.to_string())).await?
}

/// 把日志分析的筛选结果写成文本文件。
///
/// 内容由前端根据当前筛选生成，这个命令只负责在用户选定路径后写盘。
#[tauri::command]
pub async fn save_log_export(path: String, content: String) -> Result<(), String> {
    if path.trim().is_empty() {
        return Err("没有提供导出路径。".to_string());
    }

    let path = PathBuf::from(path);

    run_blocking("日志导出", move || {
        fs::write(&path, content).map_err(|error| format!("写入日志导出失败：{error}"))
    })
    .await?
}

/// 调用系统打印面板打印当前窗口（可另存为 PDF）。
#[tauri::command]
pub fn print_current_window(window: tauri::WebviewWindow) -> Result<(), String> {
    window.print().map_err(|error| format!("调用打印面板失败：{error}"))
}
