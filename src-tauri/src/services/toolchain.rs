//! 常用开发工具链检测：Python、Node.js、Git、Rust、Go、Java。
//!
//! 每个工具都按同一套策略查找：GUI PATH → 常见安装目录（含 pyenv / nvm / fnm /
//! SDKMAN 等版本管理器）→ 登录 shell 的 `command -v`。找到后执行固定的版本命令，
//! 由纯函数解析版本输出。
//!
//! 检测只读、可并发，不会安装或修改任何工具。测试通过假脚本覆盖成功、非零退出
//! 和缺失三种情况，不依赖本机是否安装这些工具。

use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use crate::models::toolchain::{ToolchainInfo, ToolchainKind, ToolchainSource, ToolchainStatus};
use crate::utils::executable;
use crate::utils::fs::{home_dir, is_executable};
use crate::utils::process::{self, ProcessError};
use crate::utils::version::compare_versions;

/// 版本命令的超时。Java / Node 首次启动可能稍慢，但不应该超过这个时间。
const VERSION_TIMEOUT: Duration = Duration::from_secs(5);

/// 登录 shell 兜底的超时。
const LOGIN_SHELL_TIMEOUT: Duration = Duration::from_secs(5);

/// 错误摘要最多保留的字符数。
const MESSAGE_EXCERPT_CHARS: usize = 240;

/// 一个工具链的固定检测配置。
struct Definition {
    kind: ToolchainKind,
    name: &'static str,
    binary: &'static str,
    version_args: &'static [&'static str],
    login_shell_command: &'static str,
    parse_version: fn(&str) -> Option<String>,
}

/// 工具的展示顺序就是总览页和详情页的卡片顺序。
static DEFINITIONS: [Definition; 6] = [
    Definition {
        kind: ToolchainKind::Python,
        name: "Python",
        binary: "python3",
        version_args: &["--version"],
        login_shell_command: "command -v python3",
        parse_version: parse_python,
    },
    Definition {
        kind: ToolchainKind::Node,
        name: "Node.js",
        binary: "node",
        version_args: &["--version"],
        login_shell_command: "command -v node",
        parse_version: parse_node,
    },
    Definition {
        kind: ToolchainKind::Git,
        name: "Git",
        binary: "git",
        version_args: &["--version"],
        login_shell_command: "command -v git",
        parse_version: parse_git,
    },
    Definition {
        kind: ToolchainKind::Rust,
        name: "Rust",
        binary: "rustc",
        version_args: &["--version"],
        login_shell_command: "command -v rustc",
        parse_version: parse_rust,
    },
    Definition {
        kind: ToolchainKind::Go,
        name: "Go",
        binary: "go",
        version_args: &["version"],
        login_shell_command: "command -v go",
        parse_version: parse_go,
    },
    Definition {
        kind: ToolchainKind::Java,
        name: "Java",
        binary: "java",
        version_args: &["-version"],
        login_shell_command: "command -v java",
        parse_version: parse_java,
    },
];

/// 一个待验证的可执行文件候选。
#[derive(Debug, Clone, PartialEq, Eq)]
struct Candidate {
    path: PathBuf,
    source: ToolchainSource,
}

/// 路径发现结果：命中项，以及本次检查过的全部路径。
#[derive(Debug, Clone, PartialEq, Eq)]
struct Discovery {
    /// 所有可作为候选的可执行文件，按查找优先级排列。
    executables: Vec<Candidate>,
    checked_paths: Vec<String>,
}

/// 检测全部常用工具链。
///
/// 各工具互不依赖，用 scoped thread 并发执行；最坏情况下（多个工具都要等登录
/// shell 超时）总耗时接近单个工具，而不是线性累加。返回顺序与 `DEFINITIONS`
/// 一致，便于 UI 稳定展示。
pub fn detect_all() -> Vec<ToolchainInfo> {
    std::thread::scope(|scope| {
        let handles: Vec<_> = DEFINITIONS
            .iter()
            .map(|definition| scope.spawn(move || detect(definition)))
            .collect();

        handles
            .into_iter()
            .zip(DEFINITIONS.iter())
            .map(|(handle, definition)| {
                handle.join().unwrap_or_else(|_| {
                    failed_info(definition, None, None, "工具链检测线程异常退出。".to_string(), Vec::new())
                })
            })
            .collect()
    })
}

fn detect(definition: &Definition) -> ToolchainInfo {
    inspect(definition, discover(definition))
}

fn discover(definition: &Definition) -> Discovery {
    let candidates = candidates(definition, env::var_os("PATH").as_deref(), home_dir().as_deref());

    discover_with(candidates, || login_shell_candidates(definition))
}

/// 收集所有可执行候选；一个都没有时才执行登录 shell 兜底。
///
/// 保留全部候选是因为 `/usr/bin` 下可能存在「存在但不可用」的占位程序（例如
/// 未安装 JDK 时的 Java stub）。版本命令失败后要继续尝试后面的候选，不能被
/// 第一个坏路径挡住。
fn discover_with(mut candidates: Vec<Candidate>, login_shell_candidates: impl FnOnce() -> Vec<Candidate>) -> Discovery {
    let mut executables = executable_candidates(&candidates);

    if executables.is_empty() {
        for candidate in login_shell_candidates() {
            push_candidate(&mut candidates, candidate);
        }
        executables = executable_candidates(&candidates);
    }

    let checked_paths = candidates
        .iter()
        .map(|candidate| candidate.path.to_string_lossy().into_owned())
        .collect();

    Discovery {
        executables,
        checked_paths,
    }
}

fn executable_candidates(candidates: &[Candidate]) -> Vec<Candidate> {
    candidates
        .iter()
        .filter(|candidate| is_executable(&candidate.path))
        .cloned()
        .collect()
}

/// 生成候选路径。
///
/// 系统目录（`/usr/bin` 等）里的占位程序会排到最后：macOS 自带一个可能是
/// stub 的 `python3` / `java`，而开发者真正想用的通常是 Homebrew、pyenv、
/// nvm 等安装的版本。非系统 PATH 仍保持原顺序，尊重用户的显式配置。
fn candidates(definition: &Definition, path_var: Option<&OsStr>, home: Option<&Path>) -> Vec<Candidate> {
    let mut candidates = Vec::new();
    let mut deferred = Vec::new();

    if let Some(path_var) = path_var {
        for path in executable::path_candidates(definition.binary, Some(path_var)) {
            let candidate = Candidate {
                path,
                source: ToolchainSource::Path,
            };

            if candidate.path.parent().is_some_and(is_system_bin) {
                deferred.push(candidate);
            } else {
                push_candidate(&mut candidates, candidate);
            }
        }
    }

    for path in common_candidates(definition, home) {
        push_candidate(
            &mut candidates,
            Candidate {
                path,
                source: ToolchainSource::CommonLocation,
            },
        );
    }

    for candidate in deferred {
        push_candidate(&mut candidates, candidate);
    }

    candidates
}

fn is_system_bin(directory: &Path) -> bool {
    matches!(directory.to_str(), Some("/usr/bin" | "/bin" | "/usr/sbin" | "/sbin"))
}

fn push_candidate(candidates: &mut Vec<Candidate>, candidate: Candidate) {
    if candidates.iter().any(|existing| existing.path == candidate.path) {
        return;
    }

    candidates.push(candidate);
}

/// 常见安装目录。版本管理器（pyenv / nvm / fnm / SDKMAN）的路径也要覆盖，
/// 因为 GUI 应用的 PATH 通常不含这些目录。
fn common_candidates(definition: &Definition, home: Option<&Path>) -> Vec<PathBuf> {
    let binary = definition.binary;
    let mut paths = Vec::new();

    match definition.kind {
        ToolchainKind::Python => {
            if let Some(home) = home {
                paths.push(home.join(".pyenv/shims").join(binary));
                paths.push(home.join(".local/bin").join(binary));
                paths.push(home.join("miniconda3/bin").join(binary));
                paths.push(home.join("anaconda3/bin").join(binary));
                paths.push(home.join("opt/anaconda3/bin").join(binary));
            }
            paths.extend(join_bin(["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"], binary));
        }
        ToolchainKind::Node => {
            if let Some(home) = home {
                paths.push(home.join(".volta/bin").join(binary));
                paths.push(home.join(".asdf/shims").join(binary));
                paths.push(home.join(".local/share/fnm/aliases/default/bin").join(binary));
                paths.extend(versioned_candidates(&home.join(".nvm/versions/node"), &["bin", binary]));
                paths.extend(versioned_candidates(
                    &home.join(".local/share/fnm/node-versions"),
                    &["installation", "bin", binary],
                ));
            }
            paths.extend(join_bin(["/opt/homebrew/bin", "/usr/local/bin"], binary));
        }
        ToolchainKind::Git => {
            paths.extend(join_bin(["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"], binary));
        }
        ToolchainKind::Rust => {
            if let Some(home) = home {
                paths.push(home.join(".cargo/bin").join(binary));
            }
            paths.extend(join_bin(["/opt/homebrew/bin", "/usr/local/bin"], binary));
        }
        ToolchainKind::Go => {
            if let Some(home) = home {
                paths.push(home.join("go/bin").join(binary));
            }
            paths.extend(join_bin(["/opt/homebrew/bin", "/usr/local/go/bin", "/usr/local/bin"], binary));
        }
        ToolchainKind::Java => {
            if let Some(home) = home {
                paths.push(home.join(".sdkman/candidates/java/current/bin").join(binary));
                paths.extend(versioned_candidates(
                    &home.join("Library/Java/JavaVirtualMachines"),
                    &["Contents", "Home", "bin", binary],
                ));
            }
            paths.extend(join_bin(
                [
                    "/opt/homebrew/opt/openjdk@21/bin",
                    "/opt/homebrew/opt/openjdk@17/bin",
                    "/opt/homebrew/opt/openjdk/bin",
                    "/opt/homebrew/bin",
                    "/Applications/Android Studio.app/Contents/jbr/Contents/Home/bin",
                    "/Applications/Android Studio.app/Contents/jre/Contents/Home/bin",
                    "/usr/bin",
                ],
                binary,
            ));
            paths.extend(versioned_candidates(
                Path::new("/Library/Java/JavaVirtualMachines"),
                &["Contents", "Home", "bin", binary],
            ));
        }
    }

    paths
}

fn join_bin<const N: usize>(directories: [&str; N], binary: &str) -> Vec<PathBuf> {
    directories.iter().map(|directory| Path::new(directory).join(binary)).collect()
}

/// 扫描版本化目录（`~/.nvm/versions/node/v20.1.0`），按版本从新到旧返回。
fn versioned_candidates(root: &Path, tail: &[&str]) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };

    let mut names = entries.flatten().map(|entry| entry.file_name()).collect::<Vec<_>>();
    names.sort_by(|left, right| compare_versions(&right.to_string_lossy(), &left.to_string_lossy()));

    names
        .into_iter()
        .map(|name| {
            let mut path = root.join(name);
            for part in tail {
                path.push(part);
            }
            path
        })
        .collect()
}

/// 通过登录 shell 解析可执行文件路径，作为 GUI PATH 缺失时的兜底。
fn login_shell_candidates(definition: &Definition) -> Vec<Candidate> {
    executable::login_shell_paths(definition.binary, definition.login_shell_command, LOGIN_SHELL_TIMEOUT)
        .into_iter()
        .map(|path| Candidate {
            path,
            source: ToolchainSource::LoginShell,
        })
        .collect()
}

/// 依次尝试可执行候选，返回第一个能解析出版本的结果。
///
/// 全部失败时报告第一个候选的失败原因：它通常是最先被用户环境拾取到的那个，
/// 也最有解释价值。
fn inspect(definition: &Definition, discovery: Discovery) -> ToolchainInfo {
    let Discovery {
        executables,
        checked_paths,
    } = discovery;

    if executables.is_empty() {
        return ToolchainInfo {
            kind: definition.kind,
            name: definition.name.to_string(),
            binary: definition.binary.to_string(),
            status: ToolchainStatus::NotFound,
            version: None,
            executable_path: None,
            source: None,
            message: Some(format!(
                "未检测到 {}。请确认 {} 已加入 PATH，或安装在常见目录。",
                definition.name, definition.binary
            )),
            checked_paths,
        };
    }

    let mut first_failure: Option<(Candidate, String)> = None;
    let mut skipped = 0usize;

    for candidate in executables {
        match run_version(definition, &candidate) {
            Ok(version) => {
                return ToolchainInfo {
                    kind: definition.kind,
                    name: definition.name.to_string(),
                    binary: definition.binary.to_string(),
                    status: ToolchainStatus::Ok,
                    version: Some(version),
                    executable_path: Some(candidate.path.to_string_lossy().into_owned()),
                    source: Some(candidate.source),
                    message: None,
                    checked_paths,
                };
            }
            Err(VersionFailure::Skip) => {
                skipped += 1;
            }
            Err(VersionFailure::Error(message)) => {
                if first_failure.is_none() {
                    first_failure = Some((candidate, message));
                }
            }
        }
    }

    // macOS 的 /usr/bin/java 是占位程序，没有 JDK 时会提示安装。把它当成
    // 「没装 Java」而不是「Java 坏了」，避免给不使用 Java 的开发者报红。
    if first_failure.is_none() && skipped > 0 {
        return ToolchainInfo {
            kind: definition.kind,
            name: definition.name.to_string(),
            binary: definition.binary.to_string(),
            status: ToolchainStatus::NotFound,
            version: None,
            executable_path: None,
            source: None,
            message: Some(format!(
                "未检测到可用的 {}。系统自带的 {} 是占位程序，请安装 JDK 或通过 Homebrew / SDKMAN 安装。",
                definition.name, definition.binary
            )),
            checked_paths,
        };
    }

    let (candidate, message) = first_failure.expect("executables 非空时至少会尝试执行一次版本命令");

    failed_info(
        definition,
        Some(candidate.path.to_string_lossy().into_owned()),
        Some(candidate.source),
        message,
        checked_paths,
    )
}

/// 版本命令的两种失败：可跳过的占位程序，以及需要报告的真实失败。
enum VersionFailure {
    Skip,
    Error(String),
}

fn run_version(definition: &Definition, candidate: &Candidate) -> Result<String, VersionFailure> {
    let path_text = candidate.path.to_string_lossy().into_owned();
    let mut command = Command::new(&candidate.path);
    command.args(definition.version_args);

    match process::run_with_timeout(&mut command, VERSION_TIMEOUT) {
        Ok(output) => {
            // Java 把版本写到 stderr，其他工具通常写 stdout；合并后解析。
            let combined = format!("{}\n{}", output.stdout, output.stderr);

            if let Some(version) = (definition.parse_version)(&combined) {
                return Ok(version);
            }

            if definition.kind == ToolchainKind::Java && is_macos_java_stub(&combined) {
                return Err(VersionFailure::Skip);
            }

            Err(VersionFailure::Error(describe_version_failure(definition, &path_text, &output)))
        }
        Err(error) => Err(VersionFailure::Error(describe_process_error(definition, &path_text, &error))),
    }
}

/// 识别 macOS 没有 JDK 时的 Java 占位提示。
fn is_macos_java_stub(output: &str) -> bool {
    let lower = output.to_ascii_lowercase();

    lower.contains("unable to locate a java runtime")
        || lower.contains("no java runtime present")
        || lower.contains("requesting install")
}

fn describe_version_failure(definition: &Definition, path: &str, output: &process::ProcessOutput) -> String {
    let args = definition.version_args.join(" ");
    let excerpt = output.stderr_excerpt(MESSAGE_EXCERPT_CHARS);
    let note = if output.is_truncated() {
        "（输出已截断）"
    } else {
        ""
    };

    if output.success() {
        if excerpt.is_empty() {
            format!("已找到 {}（{path}），但 {args} 输出无法解析。{note}", definition.name)
        } else {
            format!("已找到 {}（{path}），但 {args} 输出无法解析：{excerpt}{note}", definition.name)
        }
    } else {
        let code = output
            .exit_code()
            .map(|code| format!("退出码 {code}"))
            .unwrap_or_else(|| "被信号终止".to_string());

        if excerpt.is_empty() {
            format!("执行 {} {args} 失败（{code}），没有输出。{note}", definition.binary)
        } else {
            format!("执行 {} {args} 失败（{code}）：{excerpt}{note}", definition.binary)
        }
    }
}

fn describe_process_error(definition: &Definition, path: &str, error: &ProcessError) -> String {
    match error {
        ProcessError::Timeout(_) => {
            format!("已找到 {}（{path}），但{error}，进程已终止。", definition.name)
        }
        ProcessError::Spawn(err) => format!("找到 {}（{path}），但无法执行：{err}。请检查文件权限。", definition.name),
        ProcessError::Io(err) => {
            format!("执行 {} 时读取输出失败：{err}", definition.binary)
        }
    }
}

fn failed_info(
    definition: &Definition,
    executable_path: Option<String>,
    source: Option<ToolchainSource>,
    message: String,
    checked_paths: Vec<String>,
) -> ToolchainInfo {
    ToolchainInfo {
        kind: definition.kind,
        name: definition.name.to_string(),
        binary: definition.binary.to_string(),
        status: ToolchainStatus::Error,
        version: None,
        executable_path,
        source,
        message: Some(message),
        checked_paths,
    }
}

// ---------- 版本解析（纯函数） ----------

fn parse_python(output: &str) -> Option<String> {
    first_token_after(output, "Python ").map(str::to_string)
}

fn parse_node(output: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let version = line.trim().strip_prefix('v')?;
        version
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_digit())
            .then(|| version.to_string())
    })
}

fn parse_git(output: &str) -> Option<String> {
    first_token_after(output, "git version ").map(str::to_string)
}

fn parse_rust(output: &str) -> Option<String> {
    first_token_after(output, "rustc ").map(str::to_string)
}

fn parse_go(output: &str) -> Option<String> {
    let token = first_token_after(output, "go version ")?;

    Some(token.strip_prefix("go").unwrap_or(token).to_string())
}

fn parse_java(output: &str) -> Option<String> {
    let line = output.lines().find(|line| line.contains("version \""))?;
    let start = line.find("version \"")? + "version \"".len();
    let rest = &line[start..];
    let end = rest.find('"')?;
    let version = rest[..end].trim();

    (!version.is_empty()).then(|| version.to_string())
}

/// 在任意一行中找到 `prefix` 后的第一个空白分隔 token。
fn first_token_after<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    text.lines().find_map(|line| {
        let rest = line.trim().strip_prefix(prefix)?;
        rest.split_whitespace().next()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    /// 测试用临时工具脚本。
    struct TempTool {
        dir: PathBuf,
        path: PathBuf,
    }

    impl TempTool {
        fn new(label: &str, script: &str) -> Self {
            let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
            let dir = env::temp_dir().join(format!("devscope-toolchain-test-{label}-{}-{id}", std::process::id()));
            fs::create_dir_all(&dir).unwrap();

            let path = dir.join("python3");
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

    impl Drop for TempTool {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    fn definition(kind: ToolchainKind) -> &'static Definition {
        DEFINITIONS
            .iter()
            .find(|definition| definition.kind == kind)
            .expect("工具定义应该存在")
    }

    fn discovery_for(path: &Path) -> Discovery {
        Discovery {
            executables: vec![Candidate {
                path: path.to_path_buf(),
                source: ToolchainSource::Path,
            }],
            checked_paths: vec![path.to_string_lossy().into_owned()],
        }
    }

    #[test]
    fn parses_known_version_outputs() {
        assert_eq!(parse_python("Python 3.12.4\n"), Some("3.12.4".to_string()));
        assert_eq!(parse_node("v20.11.1\n"), Some("20.11.1".to_string()));
        assert_eq!(parse_git("git version 2.39.3 (Apple Git-145)\n"), Some("2.39.3".to_string()));
        assert_eq!(parse_rust("rustc 1.76.0 (07dca489a 2024-02-04)\n"), Some("1.76.0".to_string()));
        assert_eq!(parse_go("go version go1.22.1 darwin/arm64\n"), Some("1.22.1".to_string()));
        assert_eq!(
            parse_java("openjdk version \"21.0.2\" 2024-01-16\nOpenJDK Runtime Environment\n"),
            Some("21.0.2".to_string())
        );
    }

    #[test]
    fn ignores_unrelated_output() {
        assert_eq!(parse_python("command not found"), None);
        assert_eq!(parse_node("v"), None);
        assert_eq!(parse_java("no java here"), None);
    }

    #[cfg(unix)]
    #[test]
    fn detects_version_from_executable() {
        let tool = TempTool::new("ok", "printf 'Python 3.12.4\\n'");

        let info = inspect(definition(ToolchainKind::Python), discovery_for(&tool.path));

        assert_eq!(info.status, ToolchainStatus::Ok);
        assert_eq!(info.version.as_deref(), Some("3.12.4"));
        assert_eq!(info.executable_path, Some(tool.path.to_string_lossy().into_owned()));
        assert_eq!(info.source, Some(ToolchainSource::Path));
        assert!(info.message.is_none());
    }

    #[cfg(unix)]
    #[test]
    fn reports_non_zero_exit() {
        let tool = TempTool::new("fail", "printf 'boom' >&2; exit 3");

        let info = inspect(definition(ToolchainKind::Python), discovery_for(&tool.path));

        assert_eq!(info.status, ToolchainStatus::Error);
        let message = info.message.unwrap_or_default();
        assert!(message.contains("退出码 3"), "message: {message}");
        assert!(message.contains("boom"), "message: {message}");
    }

    #[cfg(unix)]
    #[test]
    fn skips_broken_candidates_and_uses_working_one() {
        let broken = TempTool::new("broken", "printf 'Unable to locate runtime' >&2; exit 1");
        let working = TempTool::new("working", "printf 'Python 3.12.4\n'");

        let discovery = Discovery {
            executables: vec![
                Candidate {
                    path: broken.path.clone(),
                    source: ToolchainSource::CommonLocation,
                },
                Candidate {
                    path: working.path.clone(),
                    source: ToolchainSource::Path,
                },
            ],
            checked_paths: vec![
                broken.path.to_string_lossy().into_owned(),
                working.path.to_string_lossy().into_owned(),
            ],
        };

        let info = inspect(definition(ToolchainKind::Python), discovery);

        assert_eq!(info.status, ToolchainStatus::Ok);
        assert_eq!(info.version.as_deref(), Some("3.12.4"));
        assert_eq!(info.executable_path, Some(working.path.to_string_lossy().into_owned()));
    }

    #[cfg(unix)]
    #[test]
    fn treats_macos_java_stub_as_missing() {
        let stub = TempTool::new(
            "java-stub",
            "printf 'The operation could not be completed. Unable to locate a Java Runtime.\\n' >&2; exit 1",
        );

        let info = inspect(definition(ToolchainKind::Java), discovery_for(&stub.path));

        assert_eq!(info.status, ToolchainStatus::NotFound);
        assert!(info.message.unwrap_or_default().contains("占位程序"));
    }

    #[test]
    fn reports_missing_tool() {
        let discovery = Discovery {
            executables: Vec::new(),
            checked_paths: vec!["/example/python3".to_string()],
        };

        let info = inspect(definition(ToolchainKind::Python), discovery);

        assert_eq!(info.status, ToolchainStatus::NotFound);
        assert_eq!(info.checked_paths.len(), 1);
        assert!(info.message.unwrap_or_default().contains("未检测到"), "应该给出可读的未安装提示");
    }

    #[test]
    fn builds_path_and_common_candidates_in_order() {
        let candidates = candidates(
            definition(ToolchainKind::Python),
            Some(OsStr::new("/custom/bin")),
            Some(Path::new("/home/demo")),
        );

        assert_eq!(candidates[0].path, Path::new("/custom/bin/python3"));
        assert_eq!(candidates[0].source, ToolchainSource::Path);
        assert!(candidates.iter().any(|candidate| {
            candidate.path == Path::new("/home/demo/.pyenv/shims/python3")
                && candidate.source == ToolchainSource::CommonLocation
        }));
        assert_eq!(
            candidates
                .iter()
                .filter(|candidate| candidate.path == Path::new("/custom/bin/python3"))
                .count(),
            1,
            "重复候选应该被去重"
        );
    }

    #[test]
    fn defers_system_binaries_below_developer_installs() {
        let candidates = candidates(
            definition(ToolchainKind::Python),
            Some(OsStr::new("/usr/bin:/custom/bin")),
            Some(Path::new("/home/demo")),
        );

        let position = |path: &str| {
            candidates
                .iter()
                .position(|candidate| candidate.path == Path::new(path))
                .unwrap_or_else(|| panic!("缺少候选 {path}"))
        };

        assert!(position("/custom/bin/python3") < position("/home/demo/.pyenv/shims/python3"));
        assert!(
            position("/home/demo/.pyenv/shims/python3") < position("/usr/bin/python3"),
            "系统 Python 应该排在 pyenv 之后"
        );
    }

    #[test]
    fn falls_back_to_login_shell_only_when_needed() {
        let calls = std::cell::Cell::new(0);

        let discovery = discover_with(Vec::new(), || {
            calls.set(calls.get() + 1);
            Vec::new()
        });

        assert_eq!(calls.get(), 1);
        assert!(discovery.executables.is_empty());
    }
}
