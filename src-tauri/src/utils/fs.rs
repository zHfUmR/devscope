//! 文件系统小工具：HOME 目录与可执行文件判断。
//!
//! Flutter / Android / iOS 三个检测器都需要判断「候选路径是不是一个真正的
//! 可执行文件」，放在这里避免三份权限位逻辑各自漂移。

use std::env;
use std::fs;
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
}
