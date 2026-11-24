//! 诊断报告：聚合本机配置、环境检测和可选的项目 / 日志分析。
//!
//! 生成和导出分离：
//!
//! 1. `generate` 执行检测，返回结构化报告给前端预览；
//! 2. `save` 接收前端回传的同一个报告对象，序列化成 JSON 或 Markdown 写盘。
//!
//! 这样导出时不会重复执行耗时数十秒的环境检测，也保证导出的内容与预览一致。

use std::fmt;
use std::fs;
use std::io;
use std::path::Path;
use std::time::SystemTime;

use crate::models::android::{AndroidEnvironment, AndroidStatus};
use crate::models::build_log::{BuildLogAnalysis, BuildLogStatus, BuildOutcome};
use crate::models::environment::{FlutterEnvironment, FlutterStatus};
use crate::models::ios::{IosEnvironment, IosStatus};
use crate::models::project::{ProjectAnalysis, ProjectStatus};
use crate::models::report::{
    DiagnosticReport, ReportFormat, ReportOptions, ReportSummary, ReportSummaryStatus, SavedReport,
};
use crate::models::system::SystemInfo;
use crate::models::toolchain::{ToolchainInfo, ToolchainStatus};
use crate::services::cache::DetectionCache;
use crate::services::{build_log, project};
use crate::utils::fs::read_text_capped;
use crate::utils::time;

mod markdown;

/// 报告结构版本。
const SCHEMA_VERSION: u32 = 2;

/// 载入历史报告时的文件大小上限。
const MAX_REPORT_BYTES: usize = 4 * 1024 * 1024;

/// 执行一次完整诊断。
///
/// 环境与工具链结果走共享缓存，`force` 可绕过 TTL；项目和日志分析依赖用户
/// 选择的路径，不做跨路径缓存。
pub fn generate(options: &ReportOptions, cache: &DetectionCache, force: bool) -> DiagnosticReport {
    let now = SystemTime::now();
    let system = cache.system(force);
    let flutter = cache.flutter(force);
    let android = cache.android(force);
    let ios = cache.ios(force);
    let toolchains = cache.toolchains(force);
    let project_analysis = options
        .project_path
        .as_deref()
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(|path| project::analyze(Path::new(path)));
    let build_log_analysis = options
        .log_path
        .as_deref()
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(|path| build_log::analyze(Path::new(path)));

    let summary = summarize(
        &system,
        &flutter,
        &android,
        &ios,
        &toolchains,
        project_analysis.as_ref(),
        build_log_analysis.as_ref(),
    );

    DiagnosticReport {
        schema_version: SCHEMA_VERSION,
        generator: format!("DevScope {}", env!("CARGO_PKG_VERSION")),
        generated_at: time::format_rfc3339_utc(now),
        generated_at_epoch_ms: time::epoch_millis(now),
        system,
        flutter,
        android,
        ios,
        toolchains,
        project: project_analysis,
        build_log: build_log_analysis,
        summary,
    }
}

/// 计算报告摘要。单独拆成纯函数，方便用固定模型测试。
fn summarize(
    system: &SystemInfo,
    flutter: &FlutterEnvironment,
    android: &AndroidEnvironment,
    ios: &IosEnvironment,
    toolchains: &[ToolchainInfo],
    project: Option<&ProjectAnalysis>,
    build_log: Option<&BuildLogAnalysis>,
) -> ReportSummary {
    let mut highlights = Vec::new();

    highlights.push(format!(
        "本机：{} {} · {}",
        system.os_name.as_deref().unwrap_or("macOS"),
        system.os_version.as_deref().unwrap_or("版本未知"),
        system.chip.as_deref().unwrap_or("芯片未知"),
    ));

    highlights.push(format!("Flutter：{}", flutter_highlight(flutter)));
    highlights.push(format!("Android：{}", android_highlight(android)));
    highlights.push(format!("iOS / Xcode：{}", ios_highlight(ios)));

    let available_toolchains = toolchains.iter().filter(|tool| tool.status == ToolchainStatus::Ok).count();
    highlights.push(format!("工具链：{available_toolchains}/{} 个可用", toolchains.len()));

    if let Some(project) = project {
        highlights.push(format!("项目：{}", project_highlight(project)));
    }
    if let Some(build_log) = build_log {
        highlights.push(format!("构建日志：{}", build_log_highlight(build_log)));
    }

    let toolchain_errors = toolchains.iter().filter(|tool| tool.status == ToolchainStatus::Error).count();
    let issue_count = android.issues.len()
        + ios.issues.len()
        + toolchain_errors
        + project.map_or(0, |analysis| analysis.issues.len())
        + build_log.map_or(0, |analysis| analysis.error_count + analysis.warning_count);

    let status = if matches!(flutter.status, FlutterStatus::Error)
        || matches!(android.status, AndroidStatus::Error)
        || matches!(ios.status, IosStatus::Error)
        || project.is_some_and(|analysis| analysis.status == ProjectStatus::Error)
        || build_log.is_some_and(|analysis| analysis.status == BuildLogStatus::Error)
    {
        ReportSummaryStatus::Error
    } else if matches!(flutter.status, FlutterStatus::NotFound)
        || !matches!(android.status, AndroidStatus::Ok)
        || !matches!(ios.status, IosStatus::Ok)
        || project.is_some_and(|analysis| analysis.status == ProjectStatus::Warning)
        || build_log.is_some_and(|analysis| analysis.status == BuildLogStatus::Warning)
        || issue_count > 0
    {
        ReportSummaryStatus::Warning
    } else {
        ReportSummaryStatus::Ok
    };

    ReportSummary {
        status,
        issue_count,
        highlights,
    }
}

fn flutter_highlight(environment: &FlutterEnvironment) -> String {
    match environment.status {
        FlutterStatus::Ok => {
            let version = environment
                .version
                .as_ref()
                .map(|version| version.framework_version.as_str())
                .unwrap_or("版本未知");
            let channel = environment
                .version
                .as_ref()
                .and_then(|version| version.channel.as_deref())
                .unwrap_or("渠道未知");

            format!("{version} · {channel}")
        }
        FlutterStatus::NotFound => "未检测到 SDK".to_string(),
        FlutterStatus::Error => "检测失败".to_string(),
    }
}

fn android_highlight(environment: &AndroidEnvironment) -> String {
    match environment.status {
        AndroidStatus::Ok => "环境已就绪".to_string(),
        AndroidStatus::Warning => format!("{} 项待处理", environment.issues.len()),
        AndroidStatus::NotFound => "未检测到 SDK".to_string(),
        AndroidStatus::Error => "检测失败".to_string(),
    }
}

fn ios_highlight(environment: &IosEnvironment) -> String {
    match environment.status {
        IosStatus::Ok => "环境已就绪".to_string(),
        IosStatus::Warning => format!("{} 项待处理", environment.issues.len()),
        IosStatus::NotFound => "未检测到 Xcode".to_string(),
        IosStatus::Error => "检测失败".to_string(),
    }
}

fn project_highlight(project: &ProjectAnalysis) -> String {
    match project.status {
        ProjectStatus::Error => "无法分析".to_string(),
        ProjectStatus::Ok | ProjectStatus::Warning => {
            let name = project.name.as_deref().unwrap_or("未命名项目");
            let dependencies = project.dependencies.len() + project.dev_dependencies.len();

            if project.issues.is_empty() {
                format!("{name} · {dependencies} 个依赖")
            } else {
                format!("{name} · {dependencies} 个依赖 · {} 项待处理", project.issues.len())
            }
        }
    }
}

fn build_log_highlight(build_log: &BuildLogAnalysis) -> String {
    match build_log.status {
        BuildLogStatus::Error => "无法读取日志".to_string(),
        BuildLogStatus::Ok => "未发现错误或警告".to_string(),
        BuildLogStatus::Warning => {
            let outcome = match build_log.build_outcome {
                BuildOutcome::Success => "构建成功",
                BuildOutcome::Failure => "构建失败",
                BuildOutcome::Unknown => "构建结果未知",
            };

            format!("{outcome} · {} 个错误 / {} 个警告", build_log.error_count, build_log.warning_count)
        }
    }
}

/// 生成报告文件内容并写盘。
pub fn save(report: &DiagnosticReport, format: ReportFormat, path: &Path) -> Result<SavedReport, ReportError> {
    if path.as_os_str().is_empty() {
        return Err(ReportError::EmptyPath);
    }

    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
        && !parent.is_dir()
    {
        return Err(ReportError::MissingParent(parent.to_string_lossy().into_owned()));
    }

    let content = render(report, format)?;

    fs::write(path, content.as_bytes()).map_err(ReportError::Write)?;

    Ok(SavedReport {
        path: path.to_string_lossy().into_owned(),
        format,
        bytes: content.len(),
    })
}

/// 把报告渲染成字符串，供写盘或复制到剪贴板复用。
pub fn render(report: &DiagnosticReport, format: ReportFormat) -> Result<String, ReportError> {
    match format {
        ReportFormat::Json => {
            let mut json = serde_json::to_string_pretty(report)?;
            json.push('\n');
            Ok(json)
        }
        ReportFormat::Markdown => Ok(markdown::render(report)),
    }
}

/// 读取一份之前导出的 JSON 报告，用于对比。
pub fn load(path: &Path) -> Result<DiagnosticReport, ReportError> {
    if path.as_os_str().is_empty() {
        return Err(ReportError::EmptyPath);
    }

    let text = read_text_capped(path, MAX_REPORT_BYTES).map_err(ReportError::Read)?;
    if text.truncated {
        return Err(ReportError::TooLarge(MAX_REPORT_BYTES));
    }

    serde_json::from_str(&text.text).map_err(ReportError::Parse)
}

/// 导出失败的原因。
#[derive(Debug)]
pub enum ReportError {
    /// 没有提供导出路径。
    EmptyPath,
    /// 目标目录不存在。
    MissingParent(String),
    /// JSON 序列化失败。
    Serialize(serde_json::Error),
    /// 读取报告文件失败。
    Read(io::Error),
    /// 报告 JSON 无法解析。
    Parse(serde_json::Error),
    /// 报告文件超过载入上限。
    TooLarge(usize),
    /// 写文件失败。
    Write(io::Error),
}

impl fmt::Display for ReportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyPath => write!(f, "没有提供导出路径。"),
            Self::MissingParent(path) => write!(f, "导出目录不存在：{path}"),
            Self::Serialize(error) => write!(f, "生成 JSON 报告失败：{error}"),
            Self::Read(error) => write!(f, "读取报告失败：{error}"),
            Self::Parse(error) => write!(f, "报告 JSON 无法解析：{error}"),
            Self::TooLarge(bytes) => write!(f, "报告文件超过 {} MiB，无法载入。", bytes / (1024 * 1024)),
            Self::Write(error) => write!(f, "写入报告失败：{error}"),
        }
    }
}

impl std::error::Error for ReportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Serialize(error) | Self::Parse(error) => Some(error),
            Self::Read(error) | Self::Write(error) => Some(error),
            Self::EmptyPath | Self::MissingParent(_) | Self::TooLarge(_) => None,
        }
    }
}

impl From<serde_json::Error> for ReportError {
    fn from(error: serde_json::Error) -> Self {
        Self::Serialize(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::android::{AndroidSdkSource, AndroidStatus, JavaRuntime, JavaSource};
    use crate::models::build_log::{BuildLogStatus, BuildOutcome, LogEntry, LogSeverity};
    use crate::models::environment::FlutterExecutableSource;
    use crate::models::ios::{CocoaPodsInfo, CocoaPodsSource, IosStatus, SimulatorInfo};
    use crate::models::project::{
        DependencyKind, DependencySource, PlatformKind, ProjectDependency, ProjectPlatform, ProjectStatus,
    };
    use crate::models::report::ReportSummaryStatus;
    use crate::models::toolchain::{ToolchainInfo, ToolchainKind, ToolchainSource, ToolchainStatus};
    use std::env;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    fn sample_system() -> SystemInfo {
        SystemInfo {
            os_name: Some("macOS".to_string()),
            os_version: Some("14.5".to_string()),
            os_build: Some("23F79".to_string()),
            architecture: Some("arm64".to_string()),
            model: Some("Mac14,2".to_string()),
            chip: Some("Apple M2".to_string()),
            cpu_cores: Some(8),
            memory_bytes: Some(16 * 1024 * 1024 * 1024),
            hostname: Some("DevScope-Test".to_string()),
        }
    }

    fn sample_flutter() -> FlutterEnvironment {
        FlutterEnvironment {
            status: FlutterStatus::Ok,
            executable_path: Some("/Users/demo/flutter/bin/flutter".to_string()),
            executable_source: Some(FlutterExecutableSource::Path),
            version: Some(crate::models::environment::FlutterVersionInfo {
                framework_version: "3.35.3".to_string(),
                channel: Some("stable".to_string()),
                dart_sdk_version: Some("3.9.2".to_string()),
                engine_revision: None,
                framework_revision: None,
                framework_commit_date: None,
                dev_tools_version: None,
            }),
            message: None,
            checked_paths: vec!["/Users/demo/flutter/bin/flutter".to_string()],
        }
    }

    fn sample_android() -> AndroidEnvironment {
        AndroidEnvironment {
            status: AndroidStatus::Warning,
            sdk_path: Some("/Users/demo/Library/Android/sdk".to_string()),
            sdk_source: Some(AndroidSdkSource::CommonLocation),
            platforms: vec!["android-35".to_string()],
            build_tools: vec!["35.0.0".to_string()],
            cmdline_tools_version: Some("latest".to_string()),
            sdk_manager_path: None,
            adb_path: None,
            emulator_path: None,
            avds: Vec::new(),
            java: Some(JavaRuntime {
                path: "/Applications/Android Studio.app".to_string(),
                version: Some("21".to_string()),
                source: JavaSource::AndroidStudio,
            }),
            accepted_licenses: 3,
            issues: vec!["未找到 platform-tools/adb，无法连接 Android 设备。".to_string()],
            message: None,
            checked_paths: Vec::new(),
        }
    }

    fn sample_ios() -> IosEnvironment {
        IosEnvironment {
            status: IosStatus::Ok,
            developer_dir: Some("/Applications/Xcode.app/Contents/Developer".to_string()),
            xcode_path: Some("/Applications/Xcode.app".to_string()),
            xcode_version: Some("16.0".to_string()),
            xcode_build: Some("16A242d".to_string()),
            command_line_tools_path: Some("/Library/Developer/CommandLineTools".to_string()),
            license_accepted: Some(true),
            cocoapods: Some(CocoaPodsInfo {
                path: "/opt/homebrew/bin/pod".to_string(),
                version: Some("1.15.2".to_string()),
                source: CocoaPodsSource::CommonLocation,
            }),
            simulators: Some(SimulatorInfo {
                available_count: 2,
                runtimes: vec!["iOS 18.0".to_string()],
            }),
            swift_version: Some("Swift 6.0".to_string()),
            issues: Vec::new(),
            message: None,
            checked_paths: Vec::new(),
        }
    }

    fn sample_project() -> ProjectAnalysis {
        ProjectAnalysis {
            status: ProjectStatus::Warning,
            root_path: "/Users/demo/projects/demo_app".to_string(),
            name: Some("demo_app".to_string()),
            description: Some("A demo app".to_string()),
            version: Some("1.0.0+1".to_string()),
            publish_to: Some("none".to_string()),
            dart_sdk_constraint: Some("^3.5.0".to_string()),
            flutter_constraint: Some(">=3.24.0".to_string()),
            dependencies: vec![ProjectDependency {
                name: "http".to_string(),
                constraint: Some("^1.2.0".to_string()),
                kind: DependencyKind::Regular,
                source: DependencySource::Hosted,
            }],
            dev_dependencies: Vec::new(),
            dependency_overrides: Vec::new(),
            platforms: vec![ProjectPlatform {
                platform: PlatformKind::Android,
                directory: "android".to_string(),
            }],
            uses_material_design: true,
            asset_count: 1,
            assets: vec!["assets/logo.png".to_string()],
            font_families: Vec::new(),
            android: None,
            ios: None,
            metadata: None,
            issues: vec!["iOS 相关配置缺失。".to_string()],
            message: None,
        }
    }

    fn sample_build_log() -> BuildLogAnalysis {
        BuildLogAnalysis {
            status: BuildLogStatus::Warning,
            path: "/Users/demo/build.log".to_string(),
            file_name: "build.log".to_string(),
            file_size_bytes: 2048,
            analyzed_bytes: 2048,
            total_lines: 120,
            truncated: false,
            error_count: 2,
            warning_count: 1,
            entries: vec![
                LogEntry {
                    severity: LogSeverity::Error,
                    message: "error: undefined name | with pipe".to_string(),
                    count: 2,
                    first_line: 10,
                    last_line: 12,
                },
                LogEntry {
                    severity: LogSeverity::Warning,
                    message: "warning: unused variable".to_string(),
                    count: 1,
                    first_line: 20,
                    last_line: 20,
                },
            ],
            omitted_entries: 0,
            build_outcome: BuildOutcome::Failure,
            message: None,
        }
    }

    fn sample_toolchains() -> Vec<ToolchainInfo> {
        vec![
            ToolchainInfo {
                kind: ToolchainKind::Python,
                name: "Python".to_string(),
                binary: "python3".to_string(),
                status: ToolchainStatus::Ok,
                version: Some("3.12.4".to_string()),
                executable_path: Some("/opt/homebrew/bin/python3".to_string()),
                source: Some(ToolchainSource::CommonLocation),
                message: None,
                checked_paths: Vec::new(),
            },
            ToolchainInfo {
                kind: ToolchainKind::Node,
                name: "Node.js".to_string(),
                binary: "node".to_string(),
                status: ToolchainStatus::NotFound,
                version: None,
                executable_path: None,
                source: None,
                message: Some("未检测到 Node.js。".to_string()),
                checked_paths: vec!["/opt/homebrew/bin/node".to_string()],
            },
        ]
    }

    fn sample_report() -> DiagnosticReport {
        let system = sample_system();
        let flutter = sample_flutter();
        let android = sample_android();
        let ios = sample_ios();
        let toolchains = sample_toolchains();
        let project = sample_project();
        let build_log = sample_build_log();
        let summary = summarize(&system, &flutter, &android, &ios, &toolchains, Some(&project), Some(&build_log));

        DiagnosticReport {
            schema_version: SCHEMA_VERSION,
            generator: "DevScope test".to_string(),
            generated_at: "2026-01-01T00:00:00.000Z".to_string(),
            generated_at_epoch_ms: 1_767_225_600_000,
            system,
            flutter,
            android,
            ios,
            toolchains,
            project: Some(project),
            build_log: Some(build_log),
            summary,
        }
    }

    #[test]
    fn summarizes_warnings_and_issues() {
        let report = sample_report();

        assert_eq!(report.summary.status, ReportSummaryStatus::Warning);
        assert!(report.summary.issue_count >= 4);
        assert!(report.summary.highlights.iter().any(|line| line.contains("Flutter")));
        assert!(report.summary.highlights.iter().any(|line| line.contains("构建失败")));
    }

    #[test]
    fn report_round_trips_through_json() {
        let report = sample_report();
        let json = serde_json::to_string(&report).unwrap();
        let parsed: DiagnosticReport = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed, report);
    }

    #[test]
    fn markdown_contains_sections_and_escapes_cells() {
        let report = sample_report();
        let markdown = markdown::render(&report);

        assert!(markdown.contains("# DevScope 诊断报告"));
        assert!(markdown.contains("## Flutter 环境"));
        assert!(markdown.contains("## Android 环境"));
        assert!(markdown.contains("## iOS / Xcode 环境"));
        assert!(markdown.contains("## 常用工具链"));
        assert!(markdown.contains("| Python | 已安装 | 3.12.4 |"));
        assert!(markdown.contains("## Flutter 项目"));
        assert!(markdown.contains("## 构建日志"));
        assert!(markdown.contains("BUILD FAILED") || markdown.contains("构建失败"));
        assert!(markdown.contains("undefined name \\| with pipe"));
    }

    #[test]
    fn saves_json_report() {
        let report = sample_report();
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let path = env::temp_dir().join(format!("devscope-report-test-{}-{id}.json", std::process::id()));

        let saved = save(&report, ReportFormat::Json, &path).unwrap();
        let content = fs::read_to_string(&path).unwrap();
        let _ = fs::remove_file(&path);

        assert_eq!(saved.format, ReportFormat::Json);
        assert_eq!(saved.bytes, content.len());
        assert!(content.contains("\"schemaVersion\": 2"));
    }

    #[test]
    fn saves_markdown_report() {
        let report = sample_report();
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let path = env::temp_dir().join(format!("devscope-report-test-{}-{id}.md", std::process::id()));

        let saved = save(&report, ReportFormat::Markdown, &path).unwrap();
        let content = fs::read_to_string(&path).unwrap();
        let _ = fs::remove_file(&path);

        assert_eq!(saved.format, ReportFormat::Markdown);
        assert!(content.starts_with("# DevScope 诊断报告"));
    }

    #[test]
    fn loads_a_saved_report() {
        let report = sample_report();
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let path = env::temp_dir().join(format!("devscope-report-load-test-{}-{id}.json", std::process::id()));

        save(&report, ReportFormat::Json, &path).unwrap();
        let loaded = load(&path).unwrap();
        let _ = fs::remove_file(&path);

        assert_eq!(loaded, report);
    }

    #[test]
    fn rejects_invalid_report_json() {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let path = env::temp_dir().join(format!("devscope-report-invalid-test-{}-{id}.json", std::process::id()));
        fs::write(&path, "not json").unwrap();

        let error = load(&path).unwrap_err();
        let _ = fs::remove_file(&path);

        assert!(matches!(error, ReportError::Parse(_)));
    }

    #[test]
    fn rejects_missing_export_directory() {
        let report = sample_report();
        let path = env::temp_dir().join("devscope-report-missing-dir").join("report.json");

        let error = save(&report, ReportFormat::Json, &path).unwrap_err();

        assert!(matches!(error, ReportError::MissingParent(_)));
    }
}
