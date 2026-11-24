//! 构建日志分析：统计错误 / 警告、合并重复行、识别构建结果。
//!
//! 日志格式随工具链变化（Gradle、Xcode、Flutter、Dart、CocoaPods），这里不做
//! 每种工具的完整语法解析，而是用一套保守的启发式规则：
//!
//! - 先去掉 ANSI 颜色控制符，避免同一行因为颜色码不同而被拆成多组；
//! - 含 `error` / `failed` / `exception` 等词的行算错误，含 `warning` / `[!]` 等算警告；
//! - 完全相同的行合并成一组，并记录出现次数与行号；
//! - 识别 `BUILD SUCCESSFUL` / `BUILD FAILED` 等结果标记。
//!
//! 分析只读取文件，不执行构建命令。

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

use crate::models::build_log::{BuildLogAnalysis, BuildLogStatus, BuildOutcome, LogEntry, LogSeverity};

/// 最多分析的字节数。构建日志可能非常大，诊断只需要前一部分。
const MAX_ANALYZED_BYTES: usize = 16 * 1024 * 1024;

/// 最多分析的行数，避免超长单行 / 极端文件拖慢 UI。
const MAX_ANALYZED_LINES: usize = 200_000;

/// 最多展示的分组数量，避免把上千条不同错误全部塞进 IPC 响应。
const MAX_ENTRIES: usize = 50;

/// 单条消息保留的最大字符数。
const MAX_MESSAGE_CHARS: usize = 500;

/// 分析一个日志文件。
pub fn analyze(path: &Path) -> BuildLogAnalysis {
    analyze_with(path, MAX_ANALYZED_BYTES, MAX_ANALYZED_LINES)
}

fn analyze_with(path: &Path, max_bytes: usize, max_lines: usize) -> BuildLogAnalysis {
    let display_path = path.to_string_lossy().into_owned();
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "build.log".to_string());

    let metadata = match fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => metadata,
        Ok(_) => {
            return failed_analysis(
                display_path,
                file_name,
                0,
                "所选路径不是普通文件，请选择一份构建日志。".to_string(),
            );
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return failed_analysis(display_path, file_name, 0, "日志文件不存在。".to_string());
        }
        Err(error) => {
            return failed_analysis(display_path, file_name, 0, format!("无法读取日志文件信息：{error}"));
        }
    };

    let file_size = metadata.len();
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) => {
            return failed_analysis(display_path, file_name, file_size, format!("打开日志文件失败：{error}"));
        }
    };

    // 多读 1 字节，用来判断是否还有内容超过上限。
    let limit = u64::try_from(max_bytes).unwrap_or(u64::MAX).saturating_add(1);
    let mut reader = BufReader::new(file.take(limit));
    let mut buffer = Vec::new();
    let mut accumulator = Accumulator::default();
    let mut analyzed_bytes = 0usize;
    let mut total_lines = 0usize;
    let mut truncated = false;

    loop {
        buffer.clear();
        match reader.read_until(b'\n', &mut buffer) {
            Ok(0) => break,
            Ok(read) => {
                let next_bytes = analyzed_bytes + read;
                if next_bytes > max_bytes {
                    truncated = true;
                    break;
                }
                if total_lines >= max_lines {
                    truncated = true;
                    break;
                }

                analyzed_bytes = next_bytes;
                total_lines += 1;

                let raw = String::from_utf8_lossy(&buffer);
                let line = strip_ansi(raw.trim_end_matches(['\n', '\r']));
                if !line.trim().is_empty() {
                    accumulator.push(line.trim(), total_lines);
                }
            }
            Err(error) => {
                return failed_analysis(display_path, file_name, file_size, format!("读取日志文件失败：{error}"));
            }
        }
    }

    let message = if total_lines == 0 {
        Some(format!(
            "日志文件为空，或前 {} 中没有可读内容。",
            format_bytes(u64::try_from(max_bytes).unwrap_or(u64::MAX))
        ))
    } else if truncated {
        Some(format!(
            "日志超过分析上限，只分析了前 {total_lines} 行（{} 字节）。",
            format_bytes(analyzed_bytes as u64)
        ))
    } else {
        None
    };

    let status = if total_lines == 0
        || truncated
        || accumulator.error_count > 0
        || accumulator.warning_count > 0
        || accumulator.outcome == BuildOutcome::Failure
    {
        BuildLogStatus::Warning
    } else {
        BuildLogStatus::Ok
    };

    let outcome = accumulator.outcome;
    let error_count = accumulator.error_count;
    let warning_count = accumulator.warning_count;
    let (entries, omitted_entries) = accumulator.finish();

    BuildLogAnalysis {
        status,
        path: display_path,
        file_name,
        file_size_bytes: file_size,
        analyzed_bytes: analyzed_bytes as u64,
        total_lines,
        truncated,
        error_count,
        warning_count,
        entries,
        omitted_entries,
        build_outcome: outcome,
        message,
    }
}

/// 逐行累积统计结果。
#[derive(Debug, Default)]
struct Accumulator {
    entries: Vec<EntryBuilder>,
    index: HashMap<String, usize>,
    error_count: usize,
    warning_count: usize,
    outcome: BuildOutcome,
}

#[derive(Debug)]
struct EntryBuilder {
    severity: LogSeverity,
    message: String,
    count: usize,
    first_line: usize,
    last_line: usize,
}

impl Accumulator {
    fn push(&mut self, line: &str, line_number: usize) {
        self.outcome = track_outcome(line, self.outcome);

        let Some(severity) = classify(line) else {
            return;
        };

        match severity {
            LogSeverity::Error => self.error_count += 1,
            LogSeverity::Warning => self.warning_count += 1,
        }

        let message = truncate_message(line);
        if let Some(&position) = self.index.get(&message) {
            let entry = &mut self.entries[position];
            entry.count += 1;
            entry.last_line = line_number;
            return;
        }

        self.index.insert(message.clone(), self.entries.len());
        self.entries.push(EntryBuilder {
            severity,
            message,
            count: 1,
            first_line: line_number,
            last_line: line_number,
        });
    }

    /// 按「错误优先、出现次数、首次行号」排序，并限制返回数量。
    fn finish(mut self) -> (Vec<LogEntry>, usize) {
        self.entries.sort_by(|left, right| {
            severity_rank(left.severity)
                .cmp(&severity_rank(right.severity))
                .then_with(|| right.count.cmp(&left.count))
                .then_with(|| left.first_line.cmp(&right.first_line))
        });

        let total = self.entries.len();
        let entries = self
            .entries
            .into_iter()
            .take(MAX_ENTRIES)
            .map(|entry| LogEntry {
                severity: entry.severity,
                message: entry.message,
                count: entry.count,
                first_line: entry.first_line,
                last_line: entry.last_line,
            })
            .collect();

        (entries, total.saturating_sub(MAX_ENTRIES))
    }
}

fn severity_rank(severity: LogSeverity) -> u8 {
    match severity {
        LogSeverity::Error => 0,
        LogSeverity::Warning => 1,
    }
}

/// 判断一行日志属于错误、警告还是普通信息。
fn classify(line: &str) -> Option<LogSeverity> {
    let lower = line.to_ascii_lowercase();
    let error = is_error_line(line, &lower);
    let warning = is_warning_line(line, &lower);

    match (error, warning) {
        // `warning: ... error ...` 是警告内容里提到了 error，按警告处理。
        (true, true) if lower.starts_with("warning") || lower.starts_with("warn:") => Some(LogSeverity::Warning),
        (true, _) => Some(LogSeverity::Error),
        (false, true) => Some(LogSeverity::Warning),
        (false, false) => None,
    }
}

fn is_error_line(original: &str, lower: &str) -> bool {
    const PHRASES: [&str; 16] = [
        "fatal error",
        "build failed",
        "build failure",
        "compilation failed",
        "commands failed",
        "undefined symbols",
        "undefined reference",
        "cannot find",
        "could not find",
        "failed to",
        "failed with",
        "failure:",
        "what went wrong",
        "non-zero exit",
        "exit code 1",
        "not found for",
    ];

    if PHRASES.iter().any(|phrase| lower.contains(phrase)) {
        return true;
    }
    if ["error", "fatal", "exception", "failed"]
        .iter()
        .any(|word| contains_word(lower, word))
    {
        return true;
    }
    if original.ends_with("FAILED") {
        return true;
    }
    if original.contains('✗') || original.contains('❌') {
        return true;
    }
    // Gradle 的失败任务行形如 `> Task :app:assembleDebug FAILED`。
    lower.contains("task :") && lower.contains("failed")
}

fn is_warning_line(original: &str, lower: &str) -> bool {
    if contains_word(lower, "warning") || lower.starts_with("warn:") || lower.contains("[!]") {
        return true;
    }
    if original.contains('⚠') {
        return true;
    }

    // `deprecated` 常见于 Dart / Swift 的迁移提示。
    contains_word(lower, "deprecated")
}

/// 按词边界查找，避免把 `terror` 里的 `error` 当成错误。
fn contains_word(haystack: &str, word: &str) -> bool {
    let bytes = haystack.as_bytes();
    haystack.match_indices(word).any(|(index, _)| {
        let before_ok = index == 0 || !is_word_byte(bytes[index - 1]);
        let after = index + word.len();
        let after_ok = after == bytes.len() || !is_word_byte(bytes[after]);

        before_ok && after_ok
    })
}

fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// 去掉 ANSI CSI / OSC 控制序列。日志被重定向到文件后经常还能看到颜色码。
fn strip_ansi(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch != '\u{1b}' {
            if ch != '\r' {
                out.push(ch);
            }
            continue;
        }

        match chars.next() {
            Some('[') => {
                // CSI：最后一个字节在 @..~ 范围内。
                for next in chars.by_ref() {
                    if ('@'..='~').contains(&next) {
                        break;
                    }
                }
            }
            Some(']') => {
                // OSC：以 BEL 或 ESC \ 结束。
                for next in chars.by_ref() {
                    if next == '\u{7}' {
                        break;
                    }
                    if next == '\u{1b}' {
                        if chars.peek() == Some(&'\\') {
                            chars.next();
                        }
                        break;
                    }
                }
            }
            Some(other) => out.push(other),
            None => {}
        }
    }

    out
}

fn track_outcome(line: &str, current: BuildOutcome) -> BuildOutcome {
    let lower = line.to_ascii_lowercase();

    if lower.contains("build successful") || lower.contains("build succeeded") {
        BuildOutcome::Success
    } else if lower.contains("build failed") {
        BuildOutcome::Failure
    } else {
        current
    }
}

fn truncate_message(message: &str) -> String {
    if message.chars().count() <= MAX_MESSAGE_CHARS {
        return message.to_string();
    }

    let mut truncated: String = message.chars().take(MAX_MESSAGE_CHARS).collect();
    truncated.push('…');
    truncated
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} 字节")
    }
}

fn failed_analysis(path: String, file_name: String, file_size: u64, message: String) -> BuildLogAnalysis {
    BuildLogAnalysis {
        status: BuildLogStatus::Error,
        path,
        file_name,
        file_size_bytes: file_size,
        analyzed_bytes: 0,
        total_lines: 0,
        truncated: false,
        error_count: 0,
        warning_count: 0,
        entries: Vec::new(),
        omitted_entries: 0,
        build_outcome: BuildOutcome::Unknown,
        message: Some(message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    struct TempLog {
        path: std::path::PathBuf,
    }

    impl TempLog {
        fn new(label: &str, content: &str) -> Self {
            let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
            let dir = env::temp_dir().join(format!("devscope-log-test-{label}-{}-{id}", std::process::id()));
            fs::create_dir_all(&dir).unwrap();
            let path = dir.join("build.log");
            fs::write(&path, content).unwrap();

            Self { path }
        }
    }

    impl Drop for TempLog {
        fn drop(&mut self) {
            if let Some(dir) = self.path.parent() {
                let _ = fs::remove_dir_all(dir);
            }
        }
    }

    #[test]
    fn counts_and_groups_errors_and_warnings() {
        let log = TempLog::new(
            "mixed",
            "\u{1b}[31mERROR: Gradle task assembleDebug failed with exit code 1\u{1b}[0m\n\
             warning: unused variable 'x'\n\
             \x20 at lib/main.dart:10:5\n\
             ERROR: Gradle task assembleDebug failed with exit code 1\n\
             BUILD FAILED in 2s\n",
        );

        let analysis = analyze(&log.path);

        assert_eq!(analysis.status, BuildLogStatus::Warning);
        assert_eq!(analysis.error_count, 3);
        assert_eq!(analysis.warning_count, 1);
        assert_eq!(analysis.build_outcome, BuildOutcome::Failure);
        assert_eq!(analysis.total_lines, 5);

        let errors: Vec<&LogEntry> = analysis
            .entries
            .iter()
            .filter(|entry| entry.severity == LogSeverity::Error)
            .collect();
        assert_eq!(errors.len(), 2);
        assert_eq!(errors[0].count, 2);
        assert!(!errors[0].message.contains('\u{1b}'));

        let repeated = errors.iter().find(|entry| entry.count == 2).expect("重复的错误行应该合并");
        assert_eq!(repeated.first_line, 1);
        assert_eq!(repeated.last_line, 4);
    }

    #[test]
    fn detects_successful_build_without_errors() {
        let log = TempLog::new("success", "Running Gradle task...\nBUILD SUCCESSFUL in 12s\n");

        let analysis = analyze(&log.path);

        assert_eq!(analysis.status, BuildLogStatus::Ok);
        assert_eq!(analysis.build_outcome, BuildOutcome::Success);
        assert_eq!(analysis.error_count, 0);
        assert!(analysis.entries.is_empty());
    }

    #[test]
    fn warns_on_empty_file() {
        let log = TempLog::new("empty", "");

        let analysis = analyze(&log.path);

        assert_eq!(analysis.status, BuildLogStatus::Warning);
        assert_eq!(analysis.total_lines, 0);
        assert!(analysis.message.unwrap_or_default().contains("为空"));
    }

    #[test]
    fn reports_missing_file() {
        let path = env::temp_dir().join("devscope-log-test-does-not-exist.log");

        let analysis = analyze(&path);

        assert_eq!(analysis.status, BuildLogStatus::Error);
        assert!(analysis.message.unwrap_or_default().contains("不存在"));
    }

    #[test]
    fn truncates_large_logs() {
        let content = "ERROR: line\n".repeat(100);
        let log = TempLog::new("truncated", &content);

        let analysis = analyze_with(&log.path, 60, 1000);

        assert!(analysis.truncated);
        assert_eq!(analysis.status, BuildLogStatus::Warning);
        assert!(analysis.total_lines < 100);
        assert!(analysis.analyzed_bytes < analysis.file_size_bytes);
        assert!(analysis.message.unwrap_or_default().contains("上限"));
    }

    #[test]
    fn ignores_words_that_merely_contain_error() {
        assert_eq!(classify("the terror of the build"), None);
        assert_eq!(classify("errors: 0"), None);
        assert_eq!(classify("0 failed"), Some(LogSeverity::Error));
        assert_eq!(classify("[!] CocoaPods warning"), Some(LogSeverity::Warning));
        assert_eq!(classify("warning: unused variable 'error'"), Some(LogSeverity::Warning));
    }
}
