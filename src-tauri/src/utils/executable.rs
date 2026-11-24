//! 可执行文件发现工具。
//!
//! Flutter、CocoaPods 和常用工具链都需要同一套「PATH → 常见目录 → 登录 shell」
//! 查找逻辑。这里只负责最通用的部分：PATH 解析、登录 shell 执行、输出清洗和
//! 可执行权限过滤；各检测器仍然自己决定候选目录和来源标记。

use std::env;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use crate::utils::fs::is_executable;
use crate::utils::process;

/// macOS 默认 shell 及兼容 shell。按顺序尝试，找到结果即停止。
pub const LOGIN_SHELLS: [&str; 2] = ["/bin/zsh", "/bin/bash"];

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
