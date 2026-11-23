//! Flutter 环境检测：定位可执行文件、执行版本命令、解析 `--machine` 输出。
//!
//! 所有逻辑都不依赖 Tauri，也不要求本机真的安装 Flutter；单元测试用假脚本
//! 覆盖成功、非零退出、输出异常和超时。Tauri 命令只负责薄转发。

use std::env;
use std::ffi::OsStr;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use serde::Deserialize;

use crate::models::environment::{FlutterEnvironment, FlutterExecutableSource, FlutterStatus, FlutterVersionInfo};
use crate::utils::fs::{home_dir, is_executable};
use crate::utils::process::{self, ProcessError};

mod doctor;

pub use doctor::run_flutter_doctor;

/// flutter 可执行文件的文件名，避免在多个 join / 比较处散落字面量。
const FLUTTER_BINARY: &str = "flutter";

/// 登录 shell 是最后兜底；超时太短会误判慢启动的 shell，太长会拖慢检测。
const LOGIN_SHELL_TIMEOUT: Duration = Duration::from_secs(5);

/// macOS 默认 shell 及兼容 shell。按顺序尝试，找到可执行路径即停止。
const LOGIN_SHELLS: [&str; 2] = ["/bin/zsh", "/bin/bash"];

/// 固定命令，不拼接任何外部输入。`command -v` 不会执行 flutter，只解析路径。
const LOGIN_SHELL_COMMAND: &str = "command -v flutter";

/// 版本查询超时。首次运行时 Flutter 可能初始化自己的缓存，留出比普通命令更宽裕的时间。
const VERSION_TIMEOUT: Duration = Duration::from_secs(30);

/// 错误摘要最多保留的字符数，避免把完整日志塞进 IPC 响应。
const MESSAGE_EXCERPT_CHARS: usize = 300;

/// 关闭版本检查，避免 `flutter --version` 触发网络请求；`--machine` 输出 JSON。
const VERSION_ARGS: [&str; 3] = ["--no-version-check", "--version", "--machine"];

/// 抑制 Flutter 匿名统计，减少检测过程对外部服务的访问。
const FLUTTER_ANALYTICS_ENV: &str = "FLUTTER_SUPPRESS_ANALYTICS";

/// 一个待验证的 Flutter 可执行文件候选。
#[derive(Debug, Clone, PartialEq, Eq)]
struct FlutterCandidate {
    /// 候选的绝对路径。
    path: PathBuf,
    /// 这个路径是从哪一级查找中发现的。
    source: FlutterExecutableSource,
}

/// 路径发现的结果：命中项，以及本次检查过的全部路径。
#[derive(Debug, Clone, PartialEq, Eq)]
struct FlutterDiscovery {
    /// 第一个真正可执行的候选；None 表示三级查找都没有结果。
    executable: Option<FlutterCandidate>,
    /// 按查找顺序记录的路径，用于向用户解释「检查过哪里」。
    checked_paths: Vec<String>,
}

/// 按固定优先级查找 flutter：PATH → 常见安装目录 → 登录 shell。
///
/// 前两级覆盖绝大多数安装方式；只有它们都失败时才启动用户 shell，
/// 避免每次检测都为读取 shell 配置付出额外时间。
fn discover_flutter() -> FlutterDiscovery {
    let mut candidates = candidates_from_path();
    candidates.extend(common_install_candidates());

    discover_with(candidates, candidates_from_login_shell)
}

/// 在已有候选上完成「命中判断 + 登录 shell 回退」。
///
/// `login_shell_candidates` 用闭包注入，测试可以确认：
/// 静态候选命中时不会启动 shell，未命中时才会执行兜底查找。
fn discover_with(
    mut candidates: Vec<FlutterCandidate>,
    login_shell_candidates: impl FnOnce() -> Vec<FlutterCandidate>,
) -> FlutterDiscovery {
    let mut executable = first_executable(&candidates).cloned();

    if executable.is_none() {
        for candidate in login_shell_candidates() {
            push_candidate(&mut candidates, candidate);
        }
        executable = first_executable(&candidates).cloned();
    }

    let checked_paths = candidates
        .iter()
        .map(|candidate| candidate.path.to_string_lossy().into_owned())
        .collect();

    FlutterDiscovery {
        executable,
        checked_paths,
    }
}

/// 从 GUI 进程继承的 PATH 中收集候选。
///
/// macOS GUI 应用的 PATH 通常比终端短，所以这一级可能为空；
/// 不能因此假设用户没有安装 Flutter，后面还有常见目录和登录 shell。
fn candidates_from_path() -> Vec<FlutterCandidate> {
    candidates_from_path_var(env::var_os("PATH").as_deref())
}

/// 解析给定 PATH 字符串，拆成 `目录/flutter` 候选。
///
/// 只接受绝对路径：GUI 进程的当前目录没有 shell 语义，相对路径不可靠。
/// 抽出参数版本是为了测试时不用修改进程级环境变量。
fn candidates_from_path_var(path_var: Option<&OsStr>) -> Vec<FlutterCandidate> {
    let Some(path_var) = path_var else {
        return Vec::new();
    };

    let mut candidates = Vec::new();
    for directory in env::split_paths(path_var) {
        if !directory.is_absolute() {
            continue;
        }

        push_candidate(
            &mut candidates,
            FlutterCandidate {
                path: directory.join(FLUTTER_BINARY),
                source: FlutterExecutableSource::Path,
            },
        );
    }

    candidates
}

/// 常见安装目录：Android Studio 安装器、Homebrew、FVM 默认版本等。
///
/// 这些路径在 GUI 的 PATH 里通常不存在，但用户实际安装了 Flutter 时很可能命中。
fn common_install_candidates() -> Vec<FlutterCandidate> {
    common_install_candidates_for(home_dir().as_deref())
}

/// 用显式 home 目录生成常见候选，便于测试不依赖真实 HOME。
fn common_install_candidates_for(home: Option<&Path>) -> Vec<FlutterCandidate> {
    let mut paths = Vec::new();

    if let Some(home) = home {
        paths.push(home.join("development/flutter/bin").join(FLUTTER_BINARY));
        paths.push(home.join("flutter/bin").join(FLUTTER_BINARY));
        paths.push(home.join("fvm/default/bin").join(FLUTTER_BINARY));
    }

    paths.push(PathBuf::from("/opt/homebrew/bin").join(FLUTTER_BINARY));
    paths.push(PathBuf::from("/usr/local/bin").join(FLUTTER_BINARY));
    paths.push(PathBuf::from("/opt/flutter/bin").join(FLUTTER_BINARY));

    paths
        .into_iter()
        .map(|path| FlutterCandidate {
            path,
            source: FlutterExecutableSource::CommonLocation,
        })
        .collect()
}

/// 通过用户登录 shell 解析 flutter 路径，作为 GUI PATH 缺失时的兜底。
///
/// shell 启动时可能加载 `.zprofile` / `.zshrc`，所以设置 5 秒超时；
/// 只执行固定命令，绝不把外部输入拼进 shell 字符串。
fn candidates_from_login_shell() -> Vec<FlutterCandidate> {
    for shell in LOGIN_SHELLS {
        if !Path::new(shell).is_file() {
            continue;
        }

        let mut command = Command::new(shell);
        command.args(["-lc", LOGIN_SHELL_COMMAND]);

        let Ok(output) = process::run_with_timeout(&mut command, LOGIN_SHELL_TIMEOUT) else {
            continue;
        };
        if !output.success() {
            continue;
        }

        let candidates: Vec<FlutterCandidate> = parse_login_shell_paths(&output.stdout)
            .into_iter()
            .filter(|path| is_executable(path))
            .map(|path| FlutterCandidate {
                path,
                source: FlutterExecutableSource::LoginShell,
            })
            .collect();

        if !candidates.is_empty() {
            return candidates;
        }
    }

    Vec::new()
}

/// 从登录 shell 输出中提取绝对 flutter 路径。
///
/// shell 配置可能打印欢迎语或插件提示，因此逐行筛选；倒序返回，
/// 让 `command -v` 最后输出的真实路径优先。
fn parse_login_shell_paths(stdout: &str) -> Vec<PathBuf> {
    stdout
        .lines()
        .rev()
        .filter_map(|line| {
            let path = Path::new(line.trim());

            if path.is_absolute() && path.file_name() == Some(OsStr::new(FLUTTER_BINARY)) {
                Some(path.to_path_buf())
            } else {
                None
            }
        })
        .collect()
}

/// 按路径去重后追加候选，保留第一次出现的发现方式。
///
/// 例如 Homebrew 路径同时出现在 PATH 和常见目录时，只保留来源为 PATH 的那条。
fn push_candidate(candidates: &mut Vec<FlutterCandidate>, candidate: FlutterCandidate) {
    if candidates.iter().any(|existing| existing.path == candidate.path) {
        return;
    }

    candidates.push(candidate);
}

/// 返回第一个真正可执行的候选；候选顺序就是优先级。
fn first_executable(candidates: &[FlutterCandidate]) -> Option<&FlutterCandidate> {
    candidates.iter().find(|candidate| is_executable(&candidate.path))
}

/// 解析 `flutter --version --machine` 输出时的错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionParseError {
    /// 输出中没有可解析的 JSON 对象，或 JSON 本身不合法。
    InvalidJson(String),
    /// JSON 合法，但 `frameworkVersion` 和 `flutterVersion` 都缺失。
    MissingVersion,
}

impl fmt::Display for VersionParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson(reason) => write!(f, "版本输出不是有效的 JSON：{reason}"),
            Self::MissingVersion => write!(f, "版本输出缺少 frameworkVersion / flutterVersion 字段"),
        }
    }
}

impl std::error::Error for VersionParseError {}

/// 只反序列化关心的字段；Flutter 新增其他字段时不应导致解析失败。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MachineVersion {
    framework_version: Option<String>,
    flutter_version: Option<String>,
    channel: Option<String>,
    dart_sdk_version: Option<String>,
    engine_revision: Option<String>,
    framework_revision: Option<String>,
    framework_commit_date: Option<String>,
    dev_tools_version: Option<String>,
}

/// 解析 `flutter --version --machine` 的 stdout。
///
/// Flutter 偶尔会在 JSON 前后输出提示信息，所以先截取第一个 `{` 到最后一个 `}`。
/// 不在这里执行命令，保证可以用固定字符串覆盖各种输出形态。
pub fn parse_machine_version(raw: &str) -> Result<FlutterVersionInfo, VersionParseError> {
    let json = extract_json_object(raw)
        .ok_or_else(|| VersionParseError::InvalidJson("输出中没有找到 JSON 对象".to_string()))?;
    let machine: MachineVersion =
        serde_json::from_str(json).map_err(|err| VersionParseError::InvalidJson(format!("JSON 解析失败：{err}")))?;

    let framework_version = clean(machine.framework_version)
        .or_else(|| clean(machine.flutter_version))
        .ok_or(VersionParseError::MissingVersion)?;

    Ok(FlutterVersionInfo {
        framework_version,
        channel: clean(machine.channel),
        dart_sdk_version: clean(machine.dart_sdk_version),
        engine_revision: clean(machine.engine_revision),
        framework_revision: clean(machine.framework_revision),
        framework_commit_date: clean(machine.framework_commit_date),
        dev_tools_version: clean(machine.dev_tools_version),
    })
}

/// 截取第一个 `{` 到最后一个 `}`，容忍前后的日志或提示信息。
fn extract_json_object(raw: &str) -> Option<&str> {
    let start = raw.find('{')?;
    let end = raw.rfind('}')?;

    if start < end { Some(&raw[start..=end]) } else { None }
}

/// 去掉首尾空白；空字符串按「字段缺失」处理，统一变成 None。
fn clean(value: Option<String>) -> Option<String> {
    value.map(|text| text.trim().to_string()).filter(|text| !text.is_empty())
}

/// 检测本机 Flutter 环境，是 services 层对外的唯一入口。
///
/// 路径发现、执行版本命令、解析输出都在内部完成。没装、超时、输出异常等
/// 业务失败会体现在返回值的 `status` / `message` 上，而不是 `Err`；
/// Tauri 命令层只需要处理真正的机制性失败。
pub fn detect() -> FlutterEnvironment {
    detect_with(discover_flutter(), VERSION_TIMEOUT)
}

/// 用给定的发现结果执行检测，允许测试注入候选路径和更短的超时。
fn detect_with(discovery: FlutterDiscovery, timeout: Duration) -> FlutterEnvironment {
    let FlutterDiscovery {
        executable,
        checked_paths,
    } = discovery;

    let Some(candidate) = executable else {
        return not_found_environment(checked_paths);
    };

    inspect_candidate(&candidate, checked_paths, timeout)
}

/// 对单个候选项执行 `flutter --version --machine`，并映射成前端需要的模型。
///
/// 三种失败都在这里收敛成 `status: Error` + 可读 `message`：
/// 命令没跑起来、非零退出、输出无法解析。前端不需要理解 ProcessError / VersionParseError。
fn inspect_candidate(
    candidate: &FlutterCandidate,
    checked_paths: Vec<String>,
    timeout: Duration,
) -> FlutterEnvironment {
    let path_text = candidate.path.to_string_lossy().into_owned();
    let mut command = Command::new(&candidate.path);
    command.args(VERSION_ARGS).env(FLUTTER_ANALYTICS_ENV, "true");

    match process::run_with_timeout(&mut command, timeout) {
        Ok(output) if output.success() => match parse_machine_version(&output.stdout) {
            Ok(version) => FlutterEnvironment {
                status: FlutterStatus::Ok,
                executable_path: Some(path_text),
                executable_source: Some(candidate.source),
                version: Some(version),
                message: None,
                checked_paths,
            },
            Err(parse_error) => {
                let excerpt = output.stderr_excerpt(MESSAGE_EXCERPT_CHARS);
                let note = if output.is_truncated() {
                    "（输出已截断）"
                } else {
                    ""
                };
                let message = if excerpt.is_empty() {
                    format!("已找到 Flutter（{path_text}），但版本输出无法解析：{parse_error}。{note}")
                } else {
                    format!(
                        "已找到 Flutter（{path_text}），但版本输出无法解析：{parse_error}。\
                         输出摘要：{excerpt}{note}"
                    )
                };

                failed_environment(Some(path_text), Some(candidate.source), message, checked_paths)
            }
        },
        Ok(output) => {
            let code = output
                .exit_code()
                .map(|code| format!("退出码 {code}"))
                .unwrap_or_else(|| "被信号终止".to_string());
            let excerpt = output.stderr_excerpt(MESSAGE_EXCERPT_CHARS);
            let note = if output.is_truncated() {
                "（输出已截断）"
            } else {
                ""
            };
            let message = if excerpt.is_empty() {
                format!("执行 flutter --version 失败（{code}），没有输出。{note}")
            } else {
                format!("执行 flutter --version 失败（{code}）：{excerpt}{note}")
            };

            failed_environment(Some(path_text), Some(candidate.source), message, checked_paths)
        }
        Err(error) => {
            let message = match &error {
                ProcessError::Timeout(_) => {
                    format!("已找到 flutter（{path_text}），但{error}，进程已终止。")
                }
                ProcessError::Spawn(err) => {
                    format!("找到 flutter（{path_text}），但无法执行：{err}。请检查文件权限。")
                }
                ProcessError::Io(err) => format!("执行 flutter --version 时读取输出失败：{err}"),
            };

            failed_environment(Some(path_text), Some(candidate.source), message, checked_paths)
        }
    }
}

/// 构造「未安装」结果。这不是异常：checked_paths 让前端能展开排查信息。
fn not_found_environment(checked_paths: Vec<String>) -> FlutterEnvironment {
    FlutterEnvironment {
        status: FlutterStatus::NotFound,
        executable_path: None,
        executable_source: None,
        version: None,
        message: Some(
            "未检测到 Flutter SDK。请先安装 Flutter，或确认 flutter 已加入 PATH 或登录 shell 环境。".to_string(),
        ),
        checked_paths,
    }
}

/// 构造统一的失败结果，集中设置 status / version，避免三处重复填充。
fn failed_environment(
    executable_path: Option<String>,
    executable_source: Option<FlutterExecutableSource>,
    message: String,
    checked_paths: Vec<String>,
) -> FlutterEnvironment {
    FlutterEnvironment {
        status: FlutterStatus::Error,
        executable_path,
        executable_source,
        version: None,
        message: Some(message),
        checked_paths,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::environment::{FlutterEnvironment, FlutterExecutableSource, FlutterStatus};
    use serde_json::json;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    /// 测试用临时 flutter 脚本，Drop 时自动清理目录。
    struct TempFlutter {
        dir: PathBuf,
        path: PathBuf,
    }

    impl TempFlutter {
        /// 在系统临时目录创建名为 flutter 的可执行脚本。
        fn new(script: &str) -> Self {
            let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
            let dir = env::temp_dir().join(format!("devscope-flutter-test-{}-{id}", std::process::id()));
            fs::create_dir_all(&dir).unwrap();

            let path = dir.join(FLUTTER_BINARY);
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

        /// 构造一个指定发现方式的候选。
        fn candidate(&self, source: FlutterExecutableSource) -> FlutterCandidate {
            FlutterCandidate {
                path: self.path.clone(),
                source,
            }
        }
    }

    impl Drop for TempFlutter {
        /// 测试结束或 panic 时清理临时目录，避免 /tmp 堆积。
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    /// 把临时脚本包装成发现结果，供 detect_with 的成功/失败用例复用。
    fn discovery_for(temp: &TempFlutter, source: FlutterExecutableSource) -> FlutterDiscovery {
        FlutterDiscovery {
            executable: Some(temp.candidate(source)),
            checked_paths: vec![temp.path.to_string_lossy().into_owned()],
        }
    }

    /// 登录 shell 输出有噪声时，只提取绝对 flutter 路径，并让最后一行优先。
    #[test]
    fn parses_login_shell_paths_with_noise() {
        let stdout = "Welcome to zsh\n/opt/homebrew/bin/flutter\n/usr/local/bin/flutter\n";

        let paths = parse_login_shell_paths(stdout);

        assert_eq!(
            paths,
            vec![
                PathBuf::from("/usr/local/bin/flutter"),
                PathBuf::from("/opt/homebrew/bin/flutter"),
            ]
        );
    }

    /// 相对路径、名字不是 flutter 的路径都应被忽略。
    #[test]
    fn ignores_relative_and_non_flutter_login_shell_lines() {
        let stdout = "flutter\n./flutter\n/usr/bin/flutter-wrapper\n  /opt/flutter/bin/flutter  \n";

        let paths = parse_login_shell_paths(stdout);

        assert_eq!(paths, vec![PathBuf::from("/opt/flutter/bin/flutter")]);
    }

    /// PATH 中的绝对目录会生成「目录/flutter」，相对目录被跳过。
    #[test]
    fn builds_candidates_from_absolute_path_entries() {
        let path_var = env::join_paths([
            Path::new("/usr/bin"),
            Path::new("relative/bin"),
            Path::new("/opt/homebrew/bin"),
        ])
        .unwrap();

        let candidates = candidates_from_path_var(Some(path_var.as_os_str()));

        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].path, PathBuf::from("/usr/bin/flutter"));
        assert_eq!(candidates[0].source, FlutterExecutableSource::Path);
        assert_eq!(candidates[1].path, PathBuf::from("/opt/homebrew/bin/flutter"));
    }

    /// PATH 不存在或为空时，返回空候选而不是 panic。
    #[test]
    fn returns_no_candidates_without_path() {
        assert!(candidates_from_path_var(None).is_empty());
    }

    /// 常见目录包含 HOME 下的安装位置和系统级位置。
    #[test]
    fn includes_home_and_system_install_locations() {
        let candidates = common_install_candidates_for(Some(Path::new("/Users/tester")));

        assert_eq!(candidates.len(), 6);
        assert_eq!(candidates[0].path, PathBuf::from("/Users/tester/development/flutter/bin/flutter"));
        assert!(
            candidates
                .iter()
                .all(|candidate| { candidate.source == FlutterExecutableSource::CommonLocation })
        );
        assert!(
            candidates
                .iter()
                .any(|candidate| candidate.path.as_path() == Path::new("/opt/homebrew/bin/flutter"))
        );

        assert_eq!(common_install_candidates_for(None).len(), 3);
    }

    /// 同一路径重复出现时只保留第一条，也就是优先级更高的来源。
    #[test]
    fn deduplicates_candidates_and_keeps_first_source() {
        let path = PathBuf::from("/usr/local/bin/flutter");
        let mut candidates = Vec::new();

        push_candidate(
            &mut candidates,
            FlutterCandidate {
                path: path.clone(),
                source: FlutterExecutableSource::Path,
            },
        );
        push_candidate(
            &mut candidates,
            FlutterCandidate {
                path,
                source: FlutterExecutableSource::CommonLocation,
            },
        );

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].source, FlutterExecutableSource::Path);
    }

    /// 可执行判断要求普通文件 + 执行位，目录和缺失路径都不算。
    #[test]
    fn is_executable_checks_file_type_and_permissions() {
        let temp = TempFlutter::new("exit 0");

        assert!(is_executable(&temp.path));

        let plain = temp.dir.join("notes.txt");
        fs::write(&plain, "not executable").unwrap();

        #[cfg(unix)]
        assert!(!is_executable(&plain));
        assert!(!is_executable(&temp.dir));
        assert!(!is_executable(Path::new("/devscope/no-such-binary")));
    }

    /// 静态候选命中时直接返回，不应再启动登录 shell。
    #[test]
    fn uses_static_candidate_without_loading_login_shell() {
        let temp = TempFlutter::new("exit 0");
        let candidate = temp.candidate(FlutterExecutableSource::Path);

        let discovery = discover_with(vec![candidate.clone()], || {
            panic!("静态候选已命中，不应调用登录 shell");
        });

        assert_eq!(discovery.executable, Some(candidate));
        assert_eq!(discovery.checked_paths, vec![temp.path.to_string_lossy().into_owned()]);
    }

    /// 静态候选都不可执行时，才追加登录 shell 的结果。
    #[test]
    fn falls_back_to_login_shell_candidates() {
        let temp = TempFlutter::new("exit 0");
        let missing = FlutterCandidate {
            path: PathBuf::from("/devscope/missing/flutter"),
            source: FlutterExecutableSource::Path,
        };
        let login = temp.candidate(FlutterExecutableSource::LoginShell);

        let discovery = discover_with(vec![missing.clone()], || vec![login.clone()]);

        assert_eq!(discovery.executable, Some(login));
        assert_eq!(
            discovery.checked_paths,
            vec![
                missing.path.to_string_lossy().into_owned(),
                temp.path.to_string_lossy().into_owned(),
            ]
        );
    }

    /// 三级查找都没有结果时，返回 None 但保留完整的检查路径。
    #[test]
    fn reports_not_found_with_checked_paths() {
        let missing = FlutterCandidate {
            path: PathBuf::from("/devscope/missing/flutter"),
            source: FlutterExecutableSource::Path,
        };

        let discovery = discover_with(vec![missing.clone()], Vec::new);

        assert_eq!(discovery.executable, None);
        assert_eq!(discovery.checked_paths, vec![missing.path.to_string_lossy().into_owned()]);
    }

    const FULL_JSON: &str = r#"{
        "frameworkVersion": "3.35.3",
        "channel": "stable",
        "repositoryUrl": "https://github.com/flutter/flutter.git",
        "frameworkRevision": "abc123",
        "frameworkCommitDate": "2025-09-15 10:30:00 -0700",
        "engineRevision": "def456",
        "dartSdkVersion": "3.9.2",
        "devToolsVersion": "2.48.0",
        "flutterVersion": "3.35.3",
        "frameworkChecksum": "checksum"
    }"#;

    /// 最小可解析版本，成功用例只需要这几个字段。
    const MINIMAL_JSON: &str = r#"{"frameworkVersion":"3.35.3","channel":"stable","dartSdkVersion":"3.9.2"}"#;

    #[test]
    fn parses_complete_machine_version() {
        let version = parse_machine_version(FULL_JSON).unwrap();

        assert_eq!(version.framework_version, "3.35.3");
        assert_eq!(version.channel.as_deref(), Some("stable"));
        assert_eq!(version.dart_sdk_version.as_deref(), Some("3.9.2"));
        assert_eq!(version.engine_revision.as_deref(), Some("def456"));
        assert_eq!(version.framework_revision.as_deref(), Some("abc123"));
        assert_eq!(version.framework_commit_date.as_deref(), Some("2025-09-15 10:30:00 -0700"));
        assert_eq!(version.dev_tools_version.as_deref(), Some("2.48.0"));
    }

    #[test]
    fn tolerates_noise_around_json() {
        let raw = format!("正在检查 Flutter 更新...\n{FULL_JSON}\n检测完成");

        let version = parse_machine_version(&raw).unwrap();

        assert_eq!(version.framework_version, "3.35.3");
        assert_eq!(version.dart_sdk_version.as_deref(), Some("3.9.2"));
    }

    #[test]
    fn falls_back_to_flutter_version() {
        let raw = r#"{"flutterVersion": "3.24.5", "channel": "beta"}"#;

        let version = parse_machine_version(raw).unwrap();

        assert_eq!(version.framework_version, "3.24.5");
        assert_eq!(version.channel.as_deref(), Some("beta"));
    }

    #[test]
    fn rejects_output_without_json() {
        let error = parse_machine_version("no json here").unwrap_err();

        assert!(matches!(error, VersionParseError::InvalidJson(_)));
        assert!(error.to_string().contains("JSON"));
    }

    #[test]
    fn rejects_malformed_json() {
        let error = parse_machine_version("{ not valid json }").unwrap_err();

        assert!(matches!(error, VersionParseError::InvalidJson(_)));
    }

    #[test]
    fn rejects_json_without_version_fields() {
        let raw = r#"{"channel": "stable", "dartSdkVersion": "3.9.2"}"#;

        let error = parse_machine_version(raw).unwrap_err();

        assert_eq!(error, VersionParseError::MissingVersion);
    }

    #[test]
    fn maps_blank_optional_fields_to_none() {
        let raw = r#"{
            "frameworkVersion": "3.35.3",
            "channel": "  ",
            "dartSdkVersion": "",
            "devToolsVersion": null
        }"#;

        let version = parse_machine_version(raw).unwrap();

        assert_eq!(version.channel, None);
        assert_eq!(version.dart_sdk_version, None);
        assert_eq!(version.dev_tools_version, None);
    }

    #[test]
    fn serializes_environment_with_camel_case_keys() {
        let environment = FlutterEnvironment {
            status: FlutterStatus::Ok,
            executable_path: Some("/Users/me/development/flutter/bin/flutter".to_string()),
            executable_source: Some(FlutterExecutableSource::CommonLocation),
            version: Some(FlutterVersionInfo {
                framework_version: "3.35.3".to_string(),
                channel: Some("stable".to_string()),
                dart_sdk_version: Some("3.9.2".to_string()),
                engine_revision: Some("def456".to_string()),
                framework_revision: Some("abc123".to_string()),
                framework_commit_date: Some("2025-09-15 10:30:00 -0700".to_string()),
                dev_tools_version: Some("2.48.0".to_string()),
            }),
            message: None,
            checked_paths: vec!["/Users/me/development/flutter/bin/flutter".to_string()],
        };

        let value = serde_json::to_value(&environment).unwrap();

        assert_eq!(value["status"], json!("ok"));
        assert_eq!(value["executablePath"], json!("/Users/me/development/flutter/bin/flutter"));
        assert_eq!(value["executableSource"], json!("commonLocation"));
        assert_eq!(value["version"]["frameworkVersion"], json!("3.35.3"));
        assert_eq!(value["version"]["dartSdkVersion"], json!("3.9.2"));
        assert_eq!(value["version"]["engineRevision"], json!("def456"));
        assert_eq!(value["version"]["frameworkRevision"], json!("abc123"));
        assert_eq!(value["version"]["frameworkCommitDate"], json!("2025-09-15 10:30:00 -0700"));
        assert_eq!(value["version"]["devToolsVersion"], json!("2.48.0"));
        assert!(value["message"].is_null());
        assert_eq!(value["checkedPaths"], json!(["/Users/me/development/flutter/bin/flutter"]));
    }

    /// 成功路径：执行脚本、解析版本，并原样保留路径和发现方式。
    #[test]
    fn detects_version_and_preserves_candidate_info() {
        // 脚本会校验 T4 传入的参数和环境变量；任何一项不对都会以非零码退出。
        let script = format!(
            r#"if [ "$1" != "--no-version-check" ] || [ "$2" != "--version" ] || [ "$3" != "--machine" ]; then
  printf 'unexpected args: %s' "$*" >&2
  exit 42
fi
if [ "$FLUTTER_SUPPRESS_ANALYTICS" != "true" ]; then
  printf 'missing analytics env' >&2
  exit 43
fi
printf '%s\n' '{MINIMAL_JSON}'"#
        );
        let temp = TempFlutter::new(&script);
        let path_text = temp.path.to_string_lossy().into_owned();
        let discovery = discovery_for(&temp, FlutterExecutableSource::CommonLocation);

        let environment = detect_with(discovery, Duration::from_secs(5));

        assert_eq!(environment.status, FlutterStatus::Ok);
        assert_eq!(environment.executable_path.as_deref(), Some(path_text.as_str()));
        assert_eq!(environment.executable_source, Some(FlutterExecutableSource::CommonLocation));
        assert_eq!(environment.message, None);
        assert_eq!(environment.checked_paths, vec![path_text]);

        let version = environment.version.unwrap();
        assert_eq!(version.framework_version, "3.35.3");
        assert_eq!(version.channel.as_deref(), Some("stable"));
        assert_eq!(version.dart_sdk_version.as_deref(), Some("3.9.2"));
    }

    /// 非零退出：保留候选信息，把退出码和 stderr 放进 message。
    #[test]
    fn maps_non_zero_exit_to_readable_error() {
        let temp = TempFlutter::new("printf 'boom' >&2; exit 9");
        let discovery = discovery_for(&temp, FlutterExecutableSource::Path);

        let environment = detect_with(discovery, Duration::from_secs(5));

        assert_eq!(environment.status, FlutterStatus::Error);
        assert_eq!(environment.executable_source, Some(FlutterExecutableSource::Path));
        assert_eq!(environment.version, None);

        let message = environment.message.unwrap();
        assert!(message.contains("退出码 9"));
        assert!(message.contains("boom"));
    }

    /// 输出不是合法 JSON：检测失败，但 message 里保留原始输出摘要。
    #[test]
    fn maps_invalid_json_to_parse_error() {
        let temp = TempFlutter::new("printf 'not json'");
        let discovery = discovery_for(&temp, FlutterExecutableSource::Path);

        let environment = detect_with(discovery, Duration::from_secs(5));

        assert_eq!(environment.status, FlutterStatus::Error);
        assert_eq!(environment.version, None);

        let message = environment.message.unwrap();
        assert!(message.contains("无法解析"));
        assert!(message.contains("not json"));
    }

    /// 超时：短超时下脚本被终止，返回 Error 而不是一直等待。
    #[test]
    fn maps_timeout_to_error() {
        let temp = TempFlutter::new("exec sleep 2");
        let discovery = discovery_for(&temp, FlutterExecutableSource::Path);

        let environment = detect_with(discovery, Duration::from_millis(100));

        assert_eq!(environment.status, FlutterStatus::Error);
        assert!(environment.message.unwrap().contains("超时"));
    }

    /// 未找到：不执行命令，返回 NotFound 并保留 checked_paths。
    #[test]
    fn maps_missing_discovery_to_not_found() {
        let checked_paths = vec!["/usr/local/bin/flutter".to_string()];
        let discovery = FlutterDiscovery {
            executable: None,
            checked_paths: checked_paths.clone(),
        };

        let environment = detect_with(discovery, Duration::from_secs(1));

        assert_eq!(environment.status, FlutterStatus::NotFound);
        assert_eq!(environment.executable_path, None);
        assert_eq!(environment.executable_source, None);
        assert_eq!(environment.version, None);
        assert_eq!(environment.checked_paths, checked_paths);

        let message = environment.message.unwrap();
        assert!(message.contains("未检测到"));
    }

    /// 候选文件在执行前消失：映射为 Error，并保留已知的路径和来源。
    #[test]
    fn maps_spawn_failure_to_error() {
        let path = PathBuf::from("/devscope/no-such-flutter");
        let path_text = path.to_string_lossy().into_owned();
        let discovery = FlutterDiscovery {
            executable: Some(FlutterCandidate {
                path,
                source: FlutterExecutableSource::LoginShell,
            }),
            checked_paths: vec![path_text.clone()],
        };

        let environment = detect_with(discovery, Duration::from_secs(1));

        assert_eq!(environment.status, FlutterStatus::Error);
        assert_eq!(environment.executable_path.as_deref(), Some(path_text.as_str()));
        assert_eq!(environment.executable_source, Some(FlutterExecutableSource::LoginShell));

        let message = environment.message.unwrap();
        assert!(message.contains("无法执行"));
    }

    /// 输出超过 T2 的保留上限时，错误信息应提示内容已被截断。
    #[test]
    fn mentions_when_output_is_truncated() {
        let temp = TempFlutter::new("yes a | head -c 300000");
        let discovery = discovery_for(&temp, FlutterExecutableSource::Path);

        let environment = detect_with(discovery, Duration::from_secs(5));

        assert_eq!(environment.status, FlutterStatus::Error);
        assert!(environment.message.unwrap().contains("输出已截断"));
    }
}
