//! 可执行文件发现工具。
//!
//! Flutter、CocoaPods 和常用工具链都需要同一套「PATH → 常见目录 → 登录 shell」
//! 查找逻辑。这里只负责最通用的部分：PATH 解析、登录 shell 执行、输出清洗、
//! 可执行权限过滤和候选去重；各检测器仍然自己决定候选目录和来源标记。

use std::env;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use crate::utils::fs::is_executable;
use crate::utils::process;

/// macOS 默认 shell 及兼容 shell。按顺序尝试，找到结果即停止。
pub const LOGIN_SHELLS: [&str; 2] = ["/bin/zsh", "/bin/bash"];

/// 可以按路径去重的候选。
///
/// Flutter、Android SDK、JDK、pod 和常用工具链各自有自己的候选类型（路径 +
/// 发现方式），但「同一条路径只保留第一次出现」是同一件事。用这个 trait 把
/// 去重逻辑收敛到一处，而不是每个模块各写一遍 `iter().any(...)`。
pub trait PathCandidate {
    fn path(&self) -> &Path;
}

/// 按路径去重后追加候选，保留第一次出现的发现方式。
///
/// 例如 Homebrew 路径同时出现在 PATH 和常见目录时，只保留来源为 PATH 的那条。
pub fn push_unique<T: PathCandidate>(candidates: &mut Vec<T>, candidate: T) {
    if candidates.iter().any(|existing| existing.path() == candidate.path()) {
        return;
    }

    candidates.push(candidate);
}

/// 从 GUI 进程继承的 PATH 中收集 `目录/可执行文件名` 候选。
///
/// 只接受绝对路径：GUI 进程的当前目录没有 shell 语义，相对路径不可靠。
pub fn path_candidates(binary: &str, path_var: Option<&OsStr>) -> Vec<PathBuf> {
    let Some(path_var) = path_var else {
        return Vec::new();
    };

    env::split_paths(path_var)
        .filter(|directory| directory.is_absolute())
        .map(|directory| directory.join(binary))
        .collect()
}

/// 通过用户登录 shell 解析可执行文件路径。
///
/// `command` 必须是固定字符串，不能拼接用户输入。返回所有可执行路径，
/// 由调用方决定取第一个还是全部保留。
pub fn login_shell_paths(binary: &str, command: &str, timeout: Duration) -> Vec<PathBuf> {
    for shell in LOGIN_SHELLS {
        if !Path::new(shell).is_file() {
            continue;
        }

        let mut shell_command = Command::new(shell);
        shell_command.args(["-lc", command]);

        let Ok(output) = process::run_with_timeout(&mut shell_command, timeout) else {
            continue;
        };
        if !output.success() {
            continue;
        }

        let paths = parse_login_shell_paths(&output.stdout, binary)
            .into_iter()
            .filter(|path| is_executable(path))
            .collect::<Vec<_>>();

        if !paths.is_empty() {
            return paths;
        }
    }

    Vec::new()
}

/// 从登录 shell 输出中提取绝对路径，并只保留文件名匹配的项。
///
/// shell 配置可能打印欢迎语、插件提示或 `command -v` 的多行输出，因此逐行
/// 筛选并倒序返回，让最后输出的真实路径优先。
pub fn parse_login_shell_paths(stdout: &str, binary: &str) -> Vec<PathBuf> {
    stdout
        .lines()
        .rev()
        .filter_map(|line| {
            let path = Path::new(line.trim());

            if path.is_absolute() && path.file_name() == Some(OsStr::new(binary)) {
                Some(path.to_path_buf())
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 同一路径重复出现时只保留第一条，也就是优先级更高的来源。
    #[test]
    fn deduplicates_candidates_and_keeps_first_source() {
        #[derive(Debug, PartialEq, Eq)]
        struct Candidate {
            path: PathBuf,
            source: &'static str,
        }

        impl PathCandidate for Candidate {
            fn path(&self) -> &Path {
                &self.path
            }
        }

        let path = PathBuf::from("/usr/local/bin/flutter");
        let mut candidates = Vec::new();

        push_unique(
            &mut candidates,
            Candidate {
                path: path.clone(),
                source: "PATH",
            },
        );
        push_unique(
            &mut candidates,
            Candidate {
                path,
                source: "常见安装目录",
            },
        );

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].source, "PATH");
    }

    #[test]
    fn builds_absolute_path_candidates() {
        let paths = path_candidates("flutter", Some(OsStr::new("/usr/bin:/custom/bin:relative")));

        assert_eq!(paths, vec![PathBuf::from("/usr/bin/flutter"), PathBuf::from("/custom/bin/flutter")]);
        assert!(path_candidates("flutter", None).is_empty());
    }

    #[test]
    fn parses_login_shell_output_and_skips_noise() {
        let stdout = "Welcome to zsh\n/Users/me/.rbenv/shims/pod\n/usr/bin/pod\nnot-a-path/pod\n";

        let paths = parse_login_shell_paths(stdout, "pod");

        assert_eq!(
            paths,
            vec![
                PathBuf::from("/usr/bin/pod"),
                PathBuf::from("/Users/me/.rbenv/shims/pod")
            ]
        );
    }
}
