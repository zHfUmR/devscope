//! 子进程执行工具：带超时与输出上限。
//!
//! 环境检测需要执行 `flutter --version` 这类外部命令。GUI 应用既不能无限期等待，
//! 也不能把无界输出一次性读进内存，所以统一走这里。

use std::fmt;
use std::io::{self, Read};
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// 单条流（stdout / stderr）最多保留的字节数。
const OUTPUT_LIMIT: usize = 256 * 1024;

/// 错误摘要默认最多保留的字符数，避免把完整日志塞进 IPC 响应。
pub const MESSAGE_EXCERPT_CHARS: usize = 300;

/// 轮询子进程状态的间隔。
const POLL_INTERVAL: Duration = Duration::from_millis(10);

/// 一次成功启动的进程结果。
///
/// 注意：进程非零退出也属于这里，由调用方通过 [`ProcessOutput::success`] 判断。
/// 只有「没跑起来」或「超时」才是 [`ProcessError`]。
#[derive(Debug)]
pub struct ProcessOutput {
    pub status: ExitStatus,
    pub stdout: String,
    pub stderr: String,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
}

impl ProcessOutput {
    pub fn success(&self) -> bool {
        self.status.success()
    }

    pub fn exit_code(&self) -> Option<i32> {
        self.status.code()
    }

    /// 任一条流是否因为超过上限而被截断；调用方可据此提示输出不完整。
    pub fn is_truncated(&self) -> bool {
        self.stdout_truncated || self.stderr_truncated
    }

    /// 把结束方式描述成可拼进错误文案的短语。
    ///
    /// 正常退出给出「退出码 N」；被信号杀掉时 `status.code()` 是 `None`，
    /// 统一描述为「被信号终止」，避免各检测器各写一份。
    pub fn exit_description(&self) -> String {
        self.exit_code()
            .map(|code| format!("退出码 {code}"))
            .unwrap_or_else(|| "被信号终止".to_string())
    }

    /// 取 stderr（为空时退回 stdout）作为错误摘要，并限制字符数。
    pub fn stderr_excerpt(&self, max_chars: usize) -> String {
        let stderr = self.stderr.trim();
        let text = if stderr.is_empty() { self.stdout.trim() } else { stderr };

        truncate_chars(text, max_chars)
    }
}

/// 进程没有正常跑完的原因。
#[derive(Debug)]
pub enum ProcessError {
    /// 可执行文件不存在、没有执行权限等，进程根本没有启动。
    Spawn(io::Error),
    /// 启动后读取输出或等待状态时发生 I/O 错误。
    Io(io::Error),
    /// 超过给定时间仍未结束，进程已被强制终止。
    Timeout(Duration),
}

impl fmt::Display for ProcessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawn(err) => write!(f, "无法启动进程：{err}"),
            Self::Io(err) => write!(f, "读取进程输出失败：{err}"),
            Self::Timeout(duration) => write!(f, "进程执行超时（{}）", format_duration(*duration)),
        }
    }
}

impl std::error::Error for ProcessError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Spawn(err) | Self::Io(err) => Some(err),
            Self::Timeout(_) => None,
        }
    }
}

/// 启动进程并等待结束，超时则强制终止。
///
/// stdin 被置为 null；stdout / stderr 由独立线程读取，避免任一流写满管道导致死锁。
/// 调用方设置的 stdio 会被覆盖。
pub fn run_with_timeout(command: &mut Command, timeout: Duration) -> Result<ProcessOutput, ProcessError> {
    command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = command.spawn().map_err(ProcessError::Spawn)?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| ProcessError::Io(io::Error::other("stdout 管道不可用")))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| ProcessError::Io(io::Error::other("stderr 管道不可用")))?;

    let stdout_reader = thread::spawn(move || read_capped(stdout, OUTPUT_LIMIT));
    let stderr_reader = thread::spawn(move || read_capped(stderr, OUTPUT_LIMIT));

    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();

                // 超时时不 join 读取线程：如果子进程又派生了孙进程并持有管道，
                // join 会一直等到孙进程结束，超时就失去意义。线程会在管道关闭后自行退出。
                drop(stdout_reader);
                drop(stderr_reader);

                return Err(ProcessError::Timeout(timeout));
            }
            Ok(None) => thread::sleep(POLL_INTERVAL),
            Err(err) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(ProcessError::Io(err));
            }
        }
    };

    let (stdout_bytes, stdout_truncated) = join_reader(stdout_reader)?;
    let (stderr_bytes, stderr_truncated) = join_reader(stderr_reader)?;

    Ok(ProcessOutput {
        status,
        stdout: String::from_utf8_lossy(&stdout_bytes).into_owned(),
        stderr: String::from_utf8_lossy(&stderr_bytes).into_owned(),
        stdout_truncated,
        stderr_truncated,
    })
}

fn join_reader(handle: thread::JoinHandle<io::Result<(Vec<u8>, bool)>>) -> Result<(Vec<u8>, bool), ProcessError> {
    match handle.join() {
        Ok(Ok(result)) => Ok(result),
        Ok(Err(err)) => Err(ProcessError::Io(err)),
        Err(_) => Err(ProcessError::Io(io::Error::other("读取进程输出的线程异常退出"))),
    }
}

/// 最多保留 `limit` 字节；超出部分继续读但丢弃，防止子进程因管道写满而阻塞。
fn read_capped(mut reader: impl Read, limit: usize) -> io::Result<(Vec<u8>, bool)> {
    let mut collected = Vec::with_capacity(limit.min(8 * 1024));
    let mut truncated = false;
    let mut buffer = [0_u8; 8192];

    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }

        if collected.len() < limit {
            let remaining = limit - collected.len();
            let take = remaining.min(read);
            collected.extend_from_slice(&buffer[..take]);

            if take < read {
                truncated = true;
            }
        } else {
            truncated = true;
        }
    }

    Ok((collected, truncated))
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }

    let mut truncated: String = text.chars().take(max_chars).collect();
    truncated.push('…');
    truncated
}

fn format_duration(duration: Duration) -> String {
    if duration.as_secs() >= 1 {
        format!("{} 秒", duration.as_secs())
    } else {
        format!("{} 毫秒", duration.as_millis())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn read_capped_keeps_exact_limit_without_truncation() {
        let (bytes, truncated) = read_capped(Cursor::new(vec![b'a'; 32]), 32).unwrap();

        assert_eq!(bytes.len(), 32);
        assert!(!truncated);
    }

    #[test]
    fn read_capped_discards_excess_and_marks_truncation() {
        let (bytes, truncated) = read_capped(Cursor::new(vec![b'a'; 100]), 32).unwrap();

        assert_eq!(bytes.len(), 32);
        assert!(truncated);
    }

    #[test]
    fn run_captures_stdout_stderr_and_success() {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "printf out; printf err >&2"]);

        let output = run_with_timeout(&mut command, Duration::from_secs(5)).unwrap();

        assert!(output.success());
        assert_eq!(output.exit_code(), Some(0));
        assert_eq!(output.stdout, "out");
        assert_eq!(output.stderr, "err");
        assert!(!output.stdout_truncated);
        assert!(!output.stderr_truncated);
    }

    #[test]
    fn run_returns_non_zero_exit_instead_of_error() {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "printf problem >&2; exit 7"]);

        let output = run_with_timeout(&mut command, Duration::from_secs(5)).unwrap();

        assert!(!output.success());
        assert_eq!(output.exit_code(), Some(7));
        assert_eq!(output.stderr.trim(), "problem");
    }

    #[test]
    fn run_times_out_and_kills_process() {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "exec sleep 2"]);

        let started = Instant::now();
        let result = run_with_timeout(&mut command, Duration::from_millis(100));

        match result {
            Err(ProcessError::Timeout(duration)) => assert_eq!(duration, Duration::from_millis(100)),
            other => panic!("预期超时，实际得到：{other:?}"),
        }
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn run_reports_spawn_failure() {
        let mut command = Command::new("/devscope/no-such-binary");

        let result = run_with_timeout(&mut command, Duration::from_secs(1));

        assert!(matches!(result, Err(ProcessError::Spawn(_))));
    }

    #[test]
    fn replaces_invalid_utf8_instead_of_failing() {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "printf '\\377'"]);

        let output = run_with_timeout(&mut command, Duration::from_secs(5)).unwrap();

        assert_eq!(output.stdout, "\u{fffd}");
    }

    #[test]
    fn excerpt_falls_back_to_stdout() {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "printf '  boom  '"]);

        let output = run_with_timeout(&mut command, Duration::from_secs(5)).unwrap();

        assert_eq!(output.stderr_excerpt(10), "boom");
    }

    #[test]
    fn excerpt_truncates_by_characters() {
        assert_eq!(truncate_chars("Flutter 环境检测", 7), "Flutter…");
        assert_eq!(truncate_chars("Flutter", 10), "Flutter");
    }

    #[test]
    fn timeout_message_uses_milliseconds_for_short_timeouts() {
        assert_eq!(ProcessError::Timeout(Duration::from_millis(100)).to_string(), "进程执行超时（100 毫秒）");
        assert_eq!(ProcessError::Timeout(Duration::from_secs(30)).to_string(), "进程执行超时（30 秒）");
    }
}
