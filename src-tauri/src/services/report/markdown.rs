//! 把诊断报告渲染成 Markdown。
//!
//! Markdown 面向分享和存档，所以不追求和页面完全一致，只保证：
//! 结论、关键路径、问题列表和依赖关系都能用纯文本读懂。

use std::fmt::Write as _;

use crate::models::android::AndroidStatus;
use crate::models::build_log::{BuildLogStatus, BuildOutcome, LogSeverity};
use crate::models::environment::FlutterStatus;
use crate::models::ios::IosStatus;
use crate::models::project::{DependencyKind, DependencySource, PlatformKind, ProjectStatus};
use crate::models::report::{DiagnosticReport, ReportSummaryStatus};
use crate::models::system::SystemInfo;
use crate::models::toolchain::{ToolchainInfo, ToolchainStatus};

/// 渲染完整报告。所有写入都是不可失败的字符串拼接。
pub(super) fn render(report: &DiagnosticReport) -> String {
    let mut out = String::new();

    out.push_str("# DevScope 诊断报告\n\n");
    let _ = writeln!(
        out,
        "> 生成时间：`{}` · 工具：{} · 结构版本：{}",
        report.generated_at, report.generator, report.schema_version
    );

    push_heading(&mut out, 2, "摘要");
    let _ = writeln!(out, "- **总体结论**：{}", report_status_label(report.summary.status));
    let _ = writeln!(out, "- **问题条目**：{}", report.summary.issue_count);
    for highlight in &report.summary.highlights {
        let _ = writeln!(out, "- {highlight}");
    }

    render_system(&mut out, &report.system);
    render_flutter(&mut out, report);
    render_android(&mut out, report);
    render_ios(&mut out, report);
    render_toolchains(&mut out, &report.toolchains);
    if let Some(project) = &report.project {
        render_project(&mut out, project);
    }
    if let Some(build_log) = &report.build_log {
        render_build_log(&mut out, build_log);
    }

    out.push_str("\n---\n\n");
    out.push_str("> 本报告由 DevScope 在本地生成，检测数据没有上传到任何服务器。\n");

    out
}

fn render_system(out: &mut String, system: &SystemInfo) {
    push_heading(out, 2, "本机配置");

    let rows = [
        ("系统", join_values([system.os_name.as_deref(), system.os_version.as_deref()])),
        ("Build", or_unknown(system.os_build.as_deref())),
        ("芯片", or_unknown(system.chip.as_deref())),
        ("内存", format_memory(system.memory_bytes)),
        (
            "CPU",
            system
                .cpu_cores
                .map(|cores| format!("{cores} 核"))
                .unwrap_or_else(|| "未知".to_string()),
        ),
        ("架构", or_unknown(system.architecture.as_deref())),
        ("机型", or_unknown(system.model.as_deref())),
        ("电脑名称", or_unknown(system.hostname.as_deref())),
    ];

    push_table(
        out,
        &["项目", "值"],
        &rows
            .iter()
            .map(|(key, value)| vec![key.to_string(), value.clone()])
            .collect::<Vec<_>>(),
    );
}

fn render_flutter(out: &mut String, report: &DiagnosticReport) {
    push_heading(out, 2, "Flutter 环境");

    let environment = &report.flutter;
    push_bullet(out, "结论", flutter_status_label(environment.status));
    if let Some(path) = &environment.executable_path {
        push_code_bullet(out, "可执行文件", path);
    }
    if let Some(version) = &environment.version {
        push_bullet(out, "Flutter", &version.framework_version);
        push_optional_bullet(out, "Dart SDK", version.dart_sdk_version.as_deref());
        push_optional_bullet(out, "发布渠道", version.channel.as_deref());
        push_optional_bullet(out, "Engine Revision", version.engine_revision.as_deref());
        push_optional_bullet(out, "Framework Revision", version.framework_revision.as_deref());
    }
    if let Some(message) = &environment.message {
        push_bullet(out, "说明", message);
    }
}

fn render_android(out: &mut String, report: &DiagnosticReport) {
    push_heading(out, 2, "Android 环境");

    let environment = &report.android;
    push_bullet(out, "结论", android_status_label(environment.status));
    push_optional_code_bullet(out, "SDK 路径", environment.sdk_path.as_deref());
    push_bullet(out, "Platforms", &join_list(&environment.platforms, "未安装"));
    push_bullet(out, "Build Tools", &join_list(&environment.build_tools, "未安装"));
    if let Some(java) = &environment.java {
        let version = java.version.as_deref().unwrap_or("版本未知");
        let _ = writeln!(out, "- **JDK**：{version}（`{}`）", java.path);
    } else {
        push_bullet(out, "JDK", "未找到");
    }
    push_bullet(out, "已接受 License", &environment.accepted_licenses.to_string());
    push_bullet(
        out,
        "模拟器",
        &if environment.avds.is_empty() {
            "未配置".to_string()
        } else {
            environment.avds.join(", ")
        },
    );
    if let Some(message) = &environment.message {
        push_bullet(out, "说明", message);
    }
    push_issues(out, &environment.issues);
}

fn render_toolchains(out: &mut String, toolchains: &[ToolchainInfo]) {
    push_heading(out, 2, "常用工具链");

    if toolchains.is_empty() {
        out.push_str("未检测到常用工具链。\n");
        return;
    }

    out.push_str("| 工具 | 状态 | 版本 | 路径 |\n| --- | --- | --- | --- |\n");
    for tool in toolchains {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} |",
            cell(&tool.name),
            toolchain_status_label(tool.status),
            cell(tool.version.as_deref().unwrap_or("—")),
            cell(tool.executable_path.as_deref().unwrap_or("未找到")),
        );
    }
}

fn render_ios(out: &mut String, report: &DiagnosticReport) {
    push_heading(out, 2, "iOS / Xcode 环境");

    let environment = &report.ios;
    push_bullet(out, "结论", ios_status_label(environment.status));
    if let Some(version) = &environment.xcode_version {
        let build = environment
            .xcode_build
            .as_deref()
            .map(|build| format!("（Build {build}）"))
            .unwrap_or_default();
        let _ = writeln!(out, "- **Xcode**：{version}{build}");
    } else {
        push_bullet(out, "Xcode", "未找到");
    }
    push_optional_code_bullet(out, "Developer 目录", environment.developer_dir.as_deref());
    if let Some(cocoapods) = &environment.cocoapods {
        let version = cocoapods.version.as_deref().unwrap_or("版本未知");
        push_bullet(out, "CocoaPods", &format!("{version}（`{}`）", cocoapods.path));
    } else {
        push_bullet(out, "CocoaPods", "未安装");
    }
    if let Some(simulators) = &environment.simulators {
        let runtimes = if simulators.runtimes.is_empty() {
            "无可用 runtime".to_string()
        } else {
            simulators.runtimes.join(", ")
        };
        push_bullet(out, "模拟器", &format!("{} 台可用 · {runtimes}", simulators.available_count));
    }
    push_optional_bullet(out, "Swift", environment.swift_version.as_deref());
    if let Some(message) = &environment.message {
        push_bullet(out, "说明", message);
    }
    push_issues(out, &environment.issues);
}

fn render_project(out: &mut String, project: &crate::models::project::ProjectAnalysis) {
    push_heading(out, 2, "Flutter 项目");

    push_bullet(out, "结论", project_status_label(project.status));
    push_code_bullet(out, "项目目录", &project.root_path);
    push_optional_bullet(out, "项目名", project.name.as_deref());
    push_optional_bullet(out, "版本", project.version.as_deref());
    push_optional_bullet(out, "Dart SDK", project.dart_sdk_constraint.as_deref());
    push_optional_bullet(out, "Flutter SDK", project.flutter_constraint.as_deref());

    let platforms = if project.platforms.is_empty() {
        "未启用任何平台".to_string()
    } else {
        project
            .platforms
            .iter()
            .map(|platform| platform_label(platform.platform))
            .collect::<Vec<_>>()
            .join(", ")
    };
    push_bullet(out, "平台", &platforms);
    push_bullet(
        out,
        "资源 / 字体",
        &format!("{} 项资源 · {} 个字体家族", project.asset_count, project.font_families.len()),
    );

    if let Some(android) = &project.android {
        push_optional_bullet(out, "Android applicationId", android.application_id.as_deref());
        push_optional_bullet(out, "Android minSdk", android.min_sdk.as_deref());
        push_optional_bullet(out, "Android targetSdk", android.target_sdk.as_deref());
    }
    if let Some(ios) = &project.ios {
        push_optional_bullet(out, "iOS Bundle ID", ios.bundle_identifier.as_deref());
        push_optional_bullet(out, "iOS 部署目标", ios.deployment_target.as_deref());
    }

    push_issues(out, &project.issues);

    let dependencies = project
        .dependencies
        .iter()
        .chain(&project.dev_dependencies)
        .chain(&project.dependency_overrides)
        .collect::<Vec<_>>();

    if dependencies.is_empty() {
        return;
    }

    out.push_str("\n### 依赖\n\n");
    out.push_str("| 包名 | 约束 | 类型 | 来源 |\n| --- | --- | --- | --- |\n");
    for dependency in dependencies {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} |",
            cell(&dependency.name),
            cell(dependency.constraint.as_deref().unwrap_or("any")),
            dependency_kind_label(dependency.kind),
            dependency_source_label(dependency.source),
        );
    }
}

fn render_build_log(out: &mut String, build_log: &crate::models::build_log::BuildLogAnalysis) {
    push_heading(out, 2, "构建日志");

    push_bullet(out, "结论", build_log_status_label(build_log.status));
    push_code_bullet(out, "日志文件", &build_log.path);
    push_bullet(out, "行数", &build_log.total_lines.to_string());
    push_bullet(out, "错误", &build_log.error_count.to_string());
    push_bullet(out, "警告", &build_log.warning_count.to_string());
    push_bullet(out, "构建结果", build_outcome_label(build_log.build_outcome));
    if let Some(message) = &build_log.message {
        push_bullet(out, "说明", message);
    }

    if build_log.entries.is_empty() {
        return;
    }

    out.push_str("\n### 问题分组\n\n");
    out.push_str("| 级别 | 次数 | 行号 | 内容 |\n| --- | ---: | --- | --- |\n");
    for entry in build_log.entries.iter().take(10) {
        let line = if entry.first_line == entry.last_line {
            entry.first_line.to_string()
        } else {
            format!("{}-{}", entry.first_line, entry.last_line)
        };
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} |",
            severity_label(entry.severity),
            entry.count,
            line,
            cell(&entry.message),
        );
    }
}

fn push_heading(out: &mut String, level: usize, text: &str) {
    let _ = write!(out, "\n{} {text}\n\n", "#".repeat(level));
}

fn push_bullet(out: &mut String, key: &str, value: &str) {
    let _ = writeln!(out, "- **{key}**：{value}");
}

fn push_code_bullet(out: &mut String, key: &str, value: &str) {
    let _ = writeln!(out, "- **{key}**：`{value}`");
}

fn push_optional_bullet(out: &mut String, key: &str, value: Option<&str>) {
    if let Some(value) = value.filter(|value| !value.is_empty()) {
        push_bullet(out, key, value);
    }
}

fn push_optional_code_bullet(out: &mut String, key: &str, value: Option<&str>) {
    if let Some(value) = value.filter(|value| !value.is_empty()) {
        push_code_bullet(out, key, value);
    }
}

fn push_issues(out: &mut String, issues: &[String]) {
    if issues.is_empty() {
        return;
    }

    out.push_str("\n**待处理**\n\n");
    for issue in issues {
        let _ = writeln!(out, "- {issue}");
    }
}

fn push_table(out: &mut String, headers: &[&str], rows: &[Vec<String>]) {
    let _ = writeln!(out, "| {} |", headers.join(" | "));
    let _ = writeln!(out, "| {} |", headers.iter().map(|_| "---").collect::<Vec<_>>().join(" | "));
    for row in rows {
        let cells = row.iter().map(|value| cell(value)).collect::<Vec<_>>();
        let _ = writeln!(out, "| {} |", cells.join(" | "));
    }
}

/// 缺失字段统一显示「未知」；有值时转成可写进表格的 String。
fn or_unknown(value: Option<&str>) -> String {
    value.unwrap_or("未知").to_string()
}

/// 表格单元格转义：竖线会破坏表格，换行会破坏行结构。
fn cell(value: &str) -> String {
    value.replace('|', "\\|").replace(['\n', '\r'], " ")
}

fn join_list(items: &[String], empty: &str) -> String {
    if items.is_empty() {
        empty.to_string()
    } else {
        items.join(", ")
    }
}

fn join_values(values: [Option<&str>; 2]) -> String {
    let joined = values
        .into_iter()
        .flatten()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" ");

    if joined.is_empty() {
        "未知".to_string()
    } else {
        joined
    }
}

fn format_memory(bytes: Option<u64>) -> String {
    match bytes {
        Some(bytes) if bytes > 0 => {
            let gb = bytes as f64 / (1024.0 * 1024.0 * 1024.0);

            if gb >= 10.0 {
                format!("{gb:.0} GB")
            } else {
                format!("{gb:.1} GB")
            }
        }
        _ => "未知".to_string(),
    }
}

fn report_status_label(status: ReportSummaryStatus) -> &'static str {
    match status {
        ReportSummaryStatus::Ok => "已就绪",
        ReportSummaryStatus::Warning => "需注意",
        ReportSummaryStatus::Error => "需处理",
    }
}

fn flutter_status_label(status: FlutterStatus) -> &'static str {
    match status {
        FlutterStatus::Ok => "已就绪",
        FlutterStatus::NotFound => "未安装",
        FlutterStatus::Error => "检测异常",
    }
}

fn android_status_label(status: AndroidStatus) -> &'static str {
    match status {
        AndroidStatus::Ok => "已就绪",
        AndroidStatus::Warning => "需注意",
        AndroidStatus::NotFound => "未安装",
        AndroidStatus::Error => "检测异常",
    }
}

fn ios_status_label(status: IosStatus) -> &'static str {
    match status {
        IosStatus::Ok => "已就绪",
        IosStatus::Warning => "需注意",
        IosStatus::NotFound => "未安装",
        IosStatus::Error => "检测异常",
    }
}

fn project_status_label(status: ProjectStatus) -> &'static str {
    match status {
        ProjectStatus::Ok => "结构完整",
        ProjectStatus::Warning => "需注意",
        ProjectStatus::Error => "无法分析",
    }
}

fn toolchain_status_label(status: ToolchainStatus) -> &'static str {
    match status {
        ToolchainStatus::Ok => "已安装",
        ToolchainStatus::NotFound => "未安装",
        ToolchainStatus::Error => "检测异常",
    }
}

fn build_log_status_label(status: BuildLogStatus) -> &'static str {
    match status {
        BuildLogStatus::Ok => "未发现错误",
        BuildLogStatus::Warning => "发现问题",
        BuildLogStatus::Error => "无法读取",
    }
}

fn build_outcome_label(outcome: BuildOutcome) -> &'static str {
    match outcome {
        BuildOutcome::Success => "构建成功",
        BuildOutcome::Failure => "构建失败",
        BuildOutcome::Unknown => "未识别",
    }
}

fn severity_label(severity: LogSeverity) -> &'static str {
    match severity {
        LogSeverity::Error => "错误",
        LogSeverity::Warning => "警告",
    }
}

fn platform_label(platform: PlatformKind) -> &'static str {
    match platform {
        PlatformKind::Android => "Android",
        PlatformKind::Ios => "iOS",
        PlatformKind::Web => "Web",
        PlatformKind::Macos => "macOS",
        PlatformKind::Linux => "Linux",
        PlatformKind::Windows => "Windows",
    }
}

fn dependency_kind_label(kind: DependencyKind) -> &'static str {
    match kind {
        DependencyKind::Regular => "dependencies",
        DependencyKind::Dev => "dev_dependencies",
        DependencyKind::Override => "overrides",
    }
}

fn dependency_source_label(source: DependencySource) -> &'static str {
    match source {
        DependencySource::Hosted => "pub.dev",
        DependencySource::Sdk => "SDK",
        DependencySource::Git => "Git",
        DependencySource::Path => "本地路径",
        DependencySource::Unknown => "未知",
    }
}
