//! 文件系统小工具：HOME 目录与可执行文件判断。
//!
//! Flutter / Android / iOS 三个检测器都需要判断「候选路径是不是一个真正的
//! 可执行文件」，放在这里避免三份权限位逻辑各自漂移。

use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

/// 读取 HOME 目录。GUI 进程一般能拿到；拿不到时调用方只保留系统级候选。
pub fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME").filter(|value| !value.is_empty()).map(PathBuf::from)
}

/// 判断路径是否是普通文件且具备可执行权限。
///
/// 不调用 `canonicalize`：保留用户实际配置的路径，FVM / rbenv 等符号链接原样展示。
pub fn is_executable(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };

    metadata.is_file() && has_execute_permission(&metadata)
}

#[cfg(unix)]
fn has_execute_permission(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;

    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn has_execute_permission(_metadata: &fs::Metadata) -> bool {
    true
}

/// 读取文本文件，但最多读取 `max_bytes` 字节。
///
/// 项目配置和日志都可能被用户误选成超大文件，调用方需要知道结果是否被截断。
/// 非 UTF-8 字节用替换字符处理：配置文件里出现少量乱码时仍能尽量解析，
/// 日志则不应该因为一行二进制内容就整体失败。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CappedText {
    pub text: String,
    pub truncated: bool,
}

pub fn read_text_capped(path: &Path, max_bytes: usize) -> io::Result<CappedText> {
    let file = fs::File::open(path)?;
    // 多读 1 字节，用来判断是否还有内容被丢弃。
    let mut reader = file.take(u64::try_from(max_bytes).unwrap_or(u64::MAX).saturating_add(1));

    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes)?;

    let truncated = bytes.len() > max_bytes;
    if truncated {
        bytes.truncate(max_bytes);
    }

    Ok(CappedText {
        text: String::from_utf8_lossy(&bytes).into_owned(),
        truncated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    /// 创建临时文件/目录，Drop 时清理。
    struct TempPath {
        path: PathBuf,
    }

    impl TempPath {
        fn empty(label: &str) -> Self {
            let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
            let path = env::temp_dir().join(format!("devscope-fs-test-{label}-{}-{id}", std::process::id()));
            Self { path }
        }
    }

    impl Drop for TempPath {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.path);
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn rejects_missing_and_non_executable_files() {
        let file = TempPath::empty("plain");
        fs::write(&file.path, "text").unwrap();

        assert!(!is_executable(&file.path));
        assert!(!is_executable(Path::new("/devscope/no-such-binary")));
    }

    #[cfg(unix)]
    #[test]
    fn accepts_files_with_execute_permission() {
        use std::os::unix::fs::PermissionsExt;

        let file = TempPath::empty("exec");
        fs::write(&file.path, "#!/bin/sh\n").unwrap();
        let mut permissions = fs::metadata(&file.path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&file.path, permissions).unwrap();

        assert!(is_executable(&file.path));
    }

    #[test]
    fn reads_text_without_truncation_when_under_limit() {
        let file = TempPath::empty("read-small");
        fs::write(&file.path, "pubspec").unwrap();

        let text = read_text_capped(&file.path, 1024).unwrap();

        assert_eq!(text.text, "pubspec");
        assert!(!text.truncated);
    }

    #[test]
    fn marks_text_as_truncated_when_over_limit() {
        let file = TempPath::empty("read-large");
        fs::write(&file.path, "0123456789").unwrap();

        let text = read_text_capped(&file.path, 4).unwrap();

        assert_eq!(text.text, "0123");
        assert!(text.truncated);
    }

    #[test]
    fn replaces_invalid_utf8_instead_of_failing() {
        let file = TempPath::empty("read-invalid");
        fs::write(&file.path, [0x66, 0x80, 0x6f]).unwrap();

        let text = read_text_capped(&file.path, 16).unwrap();

        assert_eq!(text.text, "f\u{fffd}o");
    }
}
