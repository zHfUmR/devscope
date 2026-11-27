//! 执行并解析 `flutter doctor -v`。
//!
//! doctor 是 Flutter 官方提供的跨工具链体检：它会同时检查 Flutter、Android、
//! Xcode、Chrome、设备和网络。输出是面向人的文本而不是稳定 JSON，所以这里
//! 采取「轻量解析 + 保留原文」的策略：解析失败也能展示原始输出。

use std::process::Command;
use std::time::Duration;

use crate::models::doctor::{DoctorReport, DoctorSection, DoctorStatus};
use crate::utils::process::{self, MESSAGE_EXCERPT_CHARS, ProcessError, ProcessOutput};

use super::{FLUTTER_ANALYTICS_ENV, FlutterCandidate, FlutterDiscovery, discover_flutter};

/// doctor 会检查设备、网络和多个工具链，比 `--version` 慢，留出更宽裕的超时。
const DOCTOR_TIMEOUT: Duration = Duration::from_secs(60);

/// `--no-version-check` 放在子命令前，避免更新检查横幅干扰输出。
const DOCTOR_ARGS: [&str; 3] = ["--no-version-check", "doctor", "-v"];

/// 运行一次完整的 Flutter 工具链体检。
///
/// 这是 services 层入口：路径发现、进程执行、文本解析都在内部完成。
/// 业务失败（没装、超时、输出无法解析）会体现在 `DoctorReport.status` / `message`，
/// 而不是 `Err`；命令层只需要处理机制性失败。
pub fn run_flutter_doctor() -> DoctorReport {
    doctor_with(discover_flutter(), DOCTOR_TIMEOUT)
}

/// 用给定的发现结果执行体检，允许测试注入候选路径和更短的超时。
fn doctor_with(discovery: FlutterDiscovery, timeout: Duration) -> DoctorReport {
    let Some(candidate) = discovery.executable else {
        return failure_report("未检测到 Flutter SDK，无法运行 flutter doctor。".to_string());
    };

    run_doctor_for(&candidate, timeout)
}

/// 对已找到的 flutter 执行 `doctor -v` 并构造报告。
fn run_doctor_for(candidate: &FlutterCandidate, timeout: Duration) -> DoctorReport {
    let mut command = Command::new(&candidate.path);
    command.args(DOCTOR_ARGS).env(FLUTTER_ANALYTICS_ENV, "true");

    match process::run_with_timeout(&mut command, timeout) {
        Ok(output) => build_report(&output),
        Err(error) => failure_report(describe_process_error(candidate, &error)),
    }
}

/// 把进程结果转成报告。
///
/// 注意：`flutter doctor` 发现 warning/error 时也可能返回非零退出码，
/// 只要 stdout 里能解析出 section，就应该展示报告而不是当成命令失败。
fn build_report(output: &ProcessOutput) -> DoctorReport {
    let sections = parse_doctor_output(&output.stdout);
    let truncated = output.is_truncated();

    if sections.is_empty() {
        let message = if output.success() {
            "flutter doctor 没有输出可识别的检查项。".to_string()
        } else {
            let code = output.exit_description();
            let excerpt = output.stderr_excerpt(MESSAGE_EXCERPT_CHARS);

            if excerpt.is_empty() {
                format!("flutter doctor 执行失败（{code}），没有输出。")
            } else {
                format!("flutter doctor 执行失败（{code}）：{excerpt}")
            }
        };

        return DoctorReport {
            status: DoctorStatus::Error,
            sections,
            // 解析失败时保留去掉 verbose 前缀的完整输出，便于排查真实格式。
            raw_output: normalize_verbose_output(&output.stdout),
            message: Some(message),
            truncated,
        };
    }

    DoctorReport {
        status: overall_status(&sections),
        sections,
        // 只保留 section 头和 bullet/warning，过滤 artifact 日志与 JSON。
        raw_output: clean_doctor_output(&output.stdout),
        message: None,
        truncated,
    }
}

/// 构造一个没有可用检查项的整体失败报告。
fn failure_report(message: String) -> DoctorReport {
    DoctorReport {
        status: DoctorStatus::Error,
        sections: Vec::new(),
        raw_output: String::new(),
        message: Some(message),
        truncated: false,
    }
}

/// 把进程层错误翻译成用户可读的 doctor 错误。
fn describe_process_error(candidate: &FlutterCandidate, error: &ProcessError) -> String {
    let path = candidate.path.to_string_lossy();

    match error {
        ProcessError::Timeout(_) => format!("已找到 flutter（{path}），但{error}，进程已终止。"),
        ProcessError::Spawn(err) => {
            format!("找到 flutter（{path}），但无法执行：{err}。请检查文件权限。")
        }
        ProcessError::Io(err) => format!("执行 flutter doctor 时读取输出失败：{err}"),
    }
}

/// 解析 doctor 输出中的检查项。
///
/// `flutter doctor -v` 打开的是全局 verbose 日志，每行都带 `[  +10 ms]` 或
/// `[        ]` 前缀。先剥掉这层前缀，再识别 `[✓]`、`[!]`、`[✗]` 开头的行。
/// 只接受 bullet / warning 作为明细，避免把 artifact 日志、JSON 混进报告。
fn parse_doctor_output(raw: &str) -> Vec<DoctorSection> {
    let mut sections = Vec::new();
    let mut current: Option<SectionBuilder> = None;

    for line in raw.lines() {
        let normalized = strip_verbose_prefix(line);
        let trimmed = normalized.trim_start();

        if let Some((status, rest)) = parse_section_header(trimmed) {
            if let Some(builder) = current.take() {
                sections.push(builder.finish());
            }
            current = Some(SectionBuilder::new(status, rest));
            continue;
        }

        let Some(builder) = current.as_mut() else {
            continue;
        };

        if trimmed.is_empty() {
            continue;
        }

        // doctor 的标题可能换行，例如 Flutter (... locale\n    zh-Hans-CN)。
        // 在遇到第一个 bullet / warning 前，把缩进行并入标题。
        let indented = normalized.starts_with(' ') || normalized.starts_with('\t');
        if builder.in_header {
            if indented && !is_detail_start(trimmed) {
                builder.append_header(trimmed);
                continue;
            }
            builder.in_header = false;
        }

        // 非 bullet / warning 的行通常是 verbose 内部日志或 JSON，直接忽略。
        if is_detail_start(trimmed) {
            builder.push_detail(trimmed);
        }
    }

    if let Some(builder) = current {
        sections.push(builder.finish());
    }

    sections
}

/// 去掉 Flutter verbose 日志的时间前缀。
///
/// 例如：
/// - `[  +10 ms] executing: ...` → `executing: ...`
/// - `[        ] [✓] Flutter ...` → `[✓] Flutter ...`
/// - 普通行原样返回。
fn strip_verbose_prefix(line: &str) -> &str {
    let Some(rest) = line.strip_prefix('[') else {
        return line;
    };
    let Some(end) = rest.find(']') else {
        return line;
    };

    if !is_verbose_marker(&rest[..end]) {
        return line;
    }

    let after = &rest[end + 1..];
    // verbose 前缀后通常有一个分隔空格；只去掉一个，保留 doctor 明细的缩进。
    after.strip_prefix(' ').unwrap_or(after)
}

/// 判断方括号内容是否是 Flutter verbose 日志的时间标记或空标记。
fn is_verbose_marker(inside: &str) -> bool {
    let trimmed = inside.trim();
    if trimmed.is_empty() {
        return true;
    }

    let Some(value) = trimmed.strip_suffix("ms") else {
        return false;
    };
    let value = value.trim();

    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_digit() || ch == ',' || ch == '+' || ch == '-')
}

/// 把 verbose 前缀全部去掉，用于解析失败时的诊断输出。
fn normalize_verbose_output(raw: &str) -> String {
    raw.lines().map(strip_verbose_prefix).collect::<Vec<_>>().join("\n")
}

/// 从 verbose 输出中提取一份可读的 doctor 报告。
///
/// 只保留 section 头和 bullet / warning 明细，过滤掉 artifact 日志、JSON、
/// 设备列表等内部输出。这样前端「查看输出」看到的是 doctor 报告，而不是终端调试日志。
fn clean_doctor_output(raw: &str) -> String {
    raw.lines()
        .map(strip_verbose_prefix)
        .filter(|line| {
            let trimmed = line.trim_start();
            parse_section_header(trimmed).is_some() || trimmed.starts_with('•') || trimmed.starts_with('!')
        })
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
}

/// 识别 section 头部的状态前缀，返回状态和去掉前缀后的标题。
fn parse_section_header(line: &str) -> Option<(DoctorStatus, &str)> {
    let markers = [
        ("[✓]", DoctorStatus::Ok),
        ("[!]", DoctorStatus::Warning),
        ("[✗]", DoctorStatus::Error),
    ];

    for (prefix, status) in markers {
        if let Some(rest) = line.strip_prefix(prefix) {
            return Some((status, rest.trim_start()));
        }
    }

    None
}

/// 判断一行是否是明细行，而不是换行后的标题续行。
fn is_detail_start(text: &str) -> bool {
    text.starts_with('•') || text.starts_with('!')
}

/// 根据 section 状态汇总整体状态：Error 优先，其次 Warning。
fn overall_status(sections: &[DoctorSection]) -> DoctorStatus {
    if sections.iter().any(|section| section.status == DoctorStatus::Error) {
        DoctorStatus::Error
    } else if sections.iter().any(|section| section.status == DoctorStatus::Warning) {
        DoctorStatus::Warning
    } else {
        DoctorStatus::Ok
    }
}

/// 解析过程中的可变 section，finish 时再计算标题/摘要。
struct SectionBuilder {
    status: DoctorStatus,
    /// 原始标题，可能包含换行续行和末尾耗时。
    title: String,
    details: Vec<String>,
    /// 第一条以 `!` 开头的明细，warning/error 时优先作为摘要。
    warning_summary: Option<String>,
    /// 是否仍在收集换行后的标题续行。
    in_header: bool,
}

impl SectionBuilder {
    /// 用 section 头和状态创建一个 builder。
    fn new(status: DoctorStatus, title: &str) -> Self {
        Self {
            status,
            title: title.to_string(),
            details: Vec::new(),
            warning_summary: None,
            in_header: true,
        }
    }

    /// 把换行后的标题续行拼回标题。
    fn append_header(&mut self, text: &str) {
        if !self.title.is_empty() {
            self.title.push(' ');
        }
        self.title.push_str(text.trim());
    }

    /// 追加一条明细；保留 `!` 提示作为 warning 摘要候选。
    fn push_detail(&mut self, line: &str) {
        let Some(detail) = clean_detail(line) else {
            return;
        };

        if line.trim_start().starts_with('!') && self.warning_summary.is_none() {
            self.warning_summary = Some(detail.clone());
        }

        self.details.push(detail);
    }

    /// 生成最终 section：拆分标题、去掉耗时、选择一行摘要。
    fn finish(self) -> DoctorSection {
        let (title, header_summary) = split_header(&self.title);
        let summary = if self.status == DoctorStatus::Ok {
            header_summary.or_else(|| self.details.first().cloned()).unwrap_or_default()
        } else {
            self.warning_summary
                .or(header_summary)
                .or_else(|| self.details.first().cloned())
                .unwrap_or_default()
        };

        DoctorSection {
            title,
            status: self.status,
            summary,
            details: self.details,
        }
    }
}

/// 去掉 `•` / `!` 前缀和首尾空白；空行返回 None。
fn clean_detail(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    let cleaned = trimmed
        .strip_prefix('•')
        .or_else(|| trimmed.strip_prefix('!'))
        .unwrap_or(trimmed)
        .trim();

    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned.to_string())
    }
}

/// 把标题拆成「名称 + 括号里的摘要」，并去掉末尾的耗时。
///
/// 例如：
/// `Flutter (Channel stable, 3.47.3, ...) [675ms]`
/// → title `Flutter`，summary `Channel stable, 3.47.3, ...`。
fn split_header(raw: &str) -> (String, Option<String>) {
    let raw = strip_trailing_timing(raw.trim());

    if let Some((title, rest)) = raw.split_once(" (") {
        let summary = rest.trim().trim_end_matches(')').trim();
        if !summary.is_empty() {
            return (title.trim().to_string(), Some(summary.to_string()));
        }
    }

    (raw.to_string(), None)
}

/// 去掉 doctor 标题末尾的 `[675ms]` / `[1,282ms]` / `[7.5s]`。
///
/// 只在方括号内容确实是数字加 ms/s 时删除，避免误伤正文里的普通方括号。
fn strip_trailing_timing(text: &str) -> &str {
    let trimmed = text.trim_end();
    let Some(open) = trimmed.rfind('[') else {
        return trimmed;
    };

    if !trimmed.ends_with(']') {
        return trimmed;
    }

    let inner = &trimmed[open + 1..trimmed.len() - 1];
    let Some(value) = inner.strip_suffix("ms").or_else(|| inner.strip_suffix('s')) else {
        return trimmed;
    };
    let normalized = value.replace(',', "");

    if normalized.parse::<f64>().is_ok() {
        trimmed[..open].trim_end()
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::environment::FlutterExecutableSource;
    use crate::services::flutter::{FlutterCandidate, FlutterDiscovery};
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    /// 用户机器上真实 doctor 输出的精简版，覆盖换行标题、耗时和 warning 明细。
    const SAMPLE: &str = r#"[✓] Flutter (Channel stable, 3.47.3, on macOS 27.0 26A428 darwin-arm64, locale
    zh-Hans-CN) [675ms]
    • Flutter version 3.47.3 on channel stable at
      /Users/me/development/flutter
    • Dart version 3.13.3
[✓] Android toolchain - develop for Android devices (Android SDK version 36.0.0)
    [1,282ms]
    • Android SDK at /Users/me/Library/Android/sdk
[!] Xcode - develop for iOS and macOS (Xcode 27.0) [787ms]
    • Xcode at /Applications/Xcode.app/Contents/Developer
    ! CocoaPods not installed.
[✓] Connected device (2 available) [7.5s]
    • macOS (desktop) • macos • darwin-arm64
! Doctor found issues in 1 category.
"#;

    /// 真实 `flutter doctor -v` 的格式：每行都带 verbose 时间前缀，中间夹着 JSON。
    const VERBOSE_SAMPLE: &str = r#"[  +10 ms] executing: sysctl hw.optional.arm64
[   +4 ms] Exit code 0 from: sysctl hw.optional.arm64
[        ] hw.optional.arm64: 1
[ +134 ms] {
             "devices" : {
               "com.apple.CoreSimulator.SimRuntime.iOS-27-0" : [

               ]
             }
           }
[  +30 ms] [✓] Flutter (Channel stable, 3.47.3, on macOS 27.0 26A428 darwin-arm64, locale zh-Hans-CN) [596ms]
[        ]     • Flutter version 3.47.3 on channel stable at /Users/me/development/flutter
[        ]     • Dart version 3.13.3
[        ] [✓] Android toolchain - develop for Android devices (Android SDK version 36.0.0) [860ms]
[        ]     • Android SDK at /Users/me/Library/Android/sdk
[        ] [!] Xcode - develop for iOS and macOS (Xcode 27.0) [702ms]
[        ]     • Xcode at /Applications/Xcode.app/Contents/Developer
[        ]     ! CocoaPods not installed.
[        ]         CocoaPods is a package manager for iOS or macOS platform code.
[ +299 ms] executing: /usr/bin/arch -arm64e xcrun simctl list devices booted
[        ] [✓] Connected device (2 available) [6.3s]
[        ]     • macOS (desktop) • macos • darwin-arm64
[        ] ! Doctor found issues in 1 category.
[        ] "flutter doctor" took 7,183ms.
"#;

    /// 测试用临时 flutter 脚本，Drop 时自动清理目录。
    struct TempFlutter {
        dir: PathBuf,
        path: PathBuf,
    }

    impl TempFlutter {
        /// 在系统临时目录创建名为 flutter 的可执行脚本。
        fn new(script: &str) -> Self {
            let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
            let dir = env::temp_dir().join(format!("devscope-doctor-test-{}-{id}", std::process::id()));
            fs::create_dir_all(&dir).unwrap();

            let path = dir.join("flutter");
            fs::write(&path, format!("#!/bin/sh\n{script}\n")).unwrap();

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;

                let mut permissions = fs::metadata(&path).unwrap().permissions();
                permissions.set_mode(0o755);
                fs::set_permissions(&path, permissions).unwrap();
            }

            Self { dir, path }
        }
    }

    impl Drop for TempFlutter {
        /// 清理临时脚本目录。
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    /// 把临时脚本包装成 doctor 需要的发现结果。
    fn discovery_for(temp: &TempFlutter) -> FlutterDiscovery {
        FlutterDiscovery {
            executable: Some(FlutterCandidate {
                path: temp.path.clone(),
                source: FlutterExecutableSource::Path,
            }),
            checked_paths: vec![temp.path.to_string_lossy().into_owned()],
        }
    }

    /// 解析用户样例：section 数量、状态、标题、摘要和明细都要正确。
    #[test]
    fn parses_sections_and_prefers_warning_summary() {
        let sections = parse_doctor_output(SAMPLE);

        assert_eq!(sections.len(), 4);
        assert_eq!(sections[0].title, "Flutter");
        assert_eq!(sections[0].summary, "Channel stable, 3.47.3, on macOS 27.0 26A428 darwin-arm64, locale zh-Hans-CN");
        assert_eq!(sections[1].title, "Android toolchain - develop for Android devices");
        assert_eq!(sections[1].summary, "Android SDK version 36.0.0");
        assert_eq!(sections[2].title, "Xcode - develop for iOS and macOS");
        assert_eq!(sections[2].status, DoctorStatus::Warning);
        assert_eq!(sections[2].summary, "CocoaPods not installed.");
        assert!(sections[2].details.iter().any(|line| line.contains("CocoaPods")));
        assert_eq!(sections[3].summary, "2 available");
        assert_eq!(overall_status(&sections), DoctorStatus::Warning);
    }

    /// 没有 section 的普通文本应返回空列表，由调用方降级展示原文。
    #[test]
    fn ignores_output_without_sections() {
        assert!(parse_doctor_output("A new version of Flutter is available!\n").is_empty());
    }

    /// 真实 verbose 输出带时间前缀，解析前必须先剥掉。
    #[test]
    fn parses_verbose_doctor_output() {
        let sections = parse_doctor_output(VERBOSE_SAMPLE);

        assert_eq!(sections.len(), 4);
        assert_eq!(sections[0].title, "Flutter");
        assert_eq!(sections[0].status, DoctorStatus::Ok);
        assert_eq!(sections[2].title, "Xcode - develop for iOS and macOS");
        assert_eq!(sections[2].summary, "CocoaPods not installed.");
        assert_eq!(overall_status(&sections), DoctorStatus::Warning);
    }

    /// 展示给用户的输出去掉内部日志和 JSON，只保留 doctor 报告。
    #[test]
    fn cleans_verbose_output_for_display() {
        let cleaned = clean_doctor_output(VERBOSE_SAMPLE);

        assert!(cleaned.contains("[✓] Flutter"));
        assert!(cleaned.contains("• Flutter version 3.47.3"));
        assert!(cleaned.contains("CocoaPods not installed."));
        assert!(!cleaned.contains("executing:"));
        assert!(!cleaned.contains("\"devices\""));
        assert!(!cleaned.contains("took 7,183ms"));
    }

    /// 只剥 verbose 前缀，不能误伤 `[✓]` 这类 section 标记。
    #[test]
    fn strips_only_verbose_prefixes() {
        assert_eq!(strip_verbose_prefix("[  +10 ms] executing: x"), "executing: x");
        assert_eq!(strip_verbose_prefix("[        ] [✓] Flutter"), "[✓] Flutter");
        assert_eq!(strip_verbose_prefix("[ +134 ms] {"), "{");
        assert_eq!(strip_verbose_prefix("[✓] Flutter"), "[✓] Flutter");
        assert_eq!(strip_verbose_prefix("plain text"), "plain text");
    }

    /// 只删除数字加 ms/s 的耗时方括号，不误伤正文。
    #[test]
    fn strips_only_numeric_timing_suffixes() {
        assert_eq!(strip_trailing_timing("Flutter (stable) [675ms]"), "Flutter (stable)");
        assert_eq!(strip_trailing_timing("Android toolchain [1,282ms]"), "Android toolchain");
        assert_eq!(strip_trailing_timing("Connected device [7.5s]"), "Connected device");
        assert_eq!(strip_trailing_timing("Note [beta]"), "Note [beta]");
    }

    /// warning 报告即使 doctor 退出码非零，也应保留 section 和摘要。
    #[test]
    fn builds_report_from_non_zero_doctor_output() {
        let script = format!(
            r#"if [ "$1" != "--no-version-check" ] || [ "$2" != "doctor" ] || [ "$3" != "-v" ]; then
  printf 'unexpected args: %s' "$*" >&2
  exit 42
fi
if [ "$FLUTTER_SUPPRESS_ANALYTICS" != "true" ]; then
  printf 'missing analytics env' >&2
  exit 43
fi
cat <<'EOF'
{VERBOSE_SAMPLE}
EOF
exit 1"#
        );
        let temp = TempFlutter::new(&script);

        let report = doctor_with(discovery_for(&temp), Duration::from_secs(5));

        assert_eq!(report.status, DoctorStatus::Warning);
        assert_eq!(report.sections.len(), 4);
        assert_eq!(report.message, None);
        assert!(!report.truncated);
        assert!(report.raw_output.contains("[!] Xcode"));
        assert!(!report.raw_output.contains("executing:"));
    }

    /// doctor 没有可解析输出时，错误信息要包含退出码和 stderr 摘要。
    #[test]
    fn reports_failure_when_output_has_no_sections() {
        let temp = TempFlutter::new("printf 'boom' >&2; exit 2");

        let report = doctor_with(discovery_for(&temp), Duration::from_secs(5));

        assert_eq!(report.status, DoctorStatus::Error);
        assert!(report.sections.is_empty());
        let message = report.message.unwrap();
        assert!(message.contains("退出码 2"));
        assert!(message.contains("boom"));
    }

    /// doctor 超时不应无限等待，返回 Error 报告。
    #[test]
    fn reports_timeout() {
        let temp = TempFlutter::new("exec sleep 2");

        let report = doctor_with(discovery_for(&temp), Duration::from_millis(100));

        assert_eq!(report.status, DoctorStatus::Error);
        assert!(report.message.unwrap().contains("超时"));
    }

    /// 没有找到 Flutter 时直接给出可读错误，不启动进程。
    #[test]
    fn reports_missing_flutter() {
        let discovery = FlutterDiscovery {
            executable: None,
            checked_paths: Vec::new(),
        };

        let report = doctor_with(discovery, Duration::from_secs(1));

        assert_eq!(report.status, DoctorStatus::Error);
        assert!(report.message.unwrap().contains("未检测到"));
    }

    /// 报告字段必须序列化成前端约定的 camelCase。
    #[test]
    fn serializes_report_with_camel_case_keys() {
        let report = DoctorReport {
            status: DoctorStatus::Warning,
            sections: vec![DoctorSection {
                title: "Xcode - develop for iOS and macOS".to_string(),
                status: DoctorStatus::Warning,
                summary: "CocoaPods not installed.".to_string(),
                details: vec!["CocoaPods not installed.".to_string()],
            }],
            raw_output: "raw".to_string(),
            message: None,
            truncated: false,
        };

        let value = serde_json::to_value(&report).unwrap();

        assert_eq!(value["status"], serde_json::json!("warning"));
        assert_eq!(value["rawOutput"], serde_json::json!("raw"));
        assert_eq!(value["sections"][0]["status"], serde_json::json!("warning"));
        assert_eq!(value["sections"][0]["summary"], serde_json::json!("CocoaPods not installed."));
        assert!(value["message"].is_null());
        assert_eq!(value["truncated"], serde_json::json!(false));
    }
}
