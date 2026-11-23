//! iOS / Xcode 环境检测：Xcode、Command Line Tools、CocoaPods 和模拟器。
//!
//! 这一层会执行 `xcode-select`、`xcodebuild`、`xcrun simctl`、`swift` 和 `pod`，
//! 所有命令都带超时并只读，不会修改用户的 Xcode 配置或安装任何东西。

use std::collections::BTreeMap;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use serde::Deserialize;

use crate::models::ios::{CocoaPodsInfo, CocoaPodsSource, IosEnvironment, IosStatus, SimulatorInfo};
use crate::utils::fs::{home_dir, is_executable};
use crate::utils::process::{self, ProcessError, ProcessOutput};
use crate::utils::version::compare_versions;

/// Xcode 命令行工具固定位置；Xcode.app 内部也有，但系统级 CLT 单独存在。
const COMMAND_LINE_TOOLS: &str = "/Library/Developer/CommandLineTools";

/// 默认的 Xcode 安装位置；即使不存在也会记入「已检查的位置」。
const DEFAULT_XCODE_APP: &str = "/Applications/Xcode.app";

const XCODE_SELECT: &str = "/usr/bin/xcode-select";
const XCODEBUILD: &str = "/usr/bin/xcodebuild";
const XCRUN: &str = "/usr/bin/xcrun";
const SWIFT: &str = "/usr/bin/swift";

/// 交互式命令的等待上限。xcodebuild 首次运行可能较慢，给 10 秒。
const XCODEBUILD_TIMEOUT: Duration = Duration::from_secs(10);
const SIMCTL_TIMEOUT: Duration = Duration::from_secs(10);
const POD_VERSION_TIMEOUT: Duration = Duration::from_secs(10);
const SWIFT_TIMEOUT: Duration = Duration::from_secs(5);

/// 登录 shell 兜底查找 `pod` 的超时。
const LOGIN_SHELL_TIMEOUT: Duration = Duration::from_secs(5);

/// 固定命令，不拼接外部输入；只解析路径，不执行 pod。
const LOGIN_SHELL_COMMAND: &str = "command -v pod";

const LOGIN_SHELLS: [&str; 2] = ["/bin/zsh", "/bin/bash"];

/// 错误摘要最多保留的字符数。
const MESSAGE_EXCERPT_CHARS: usize = 300;

/// 一个待验证的 `pod` 候选。
#[derive(Debug, Clone, PartialEq, Eq)]
struct PodCandidate {
    path: PathBuf,
    source: CocoaPodsSource,
}

/// CocoaPods 路径发现结果。
#[derive(Debug, Clone, PartialEq, Eq)]
struct PodDiscovery {
    candidate: Option<PodCandidate>,
    checked_paths: Vec<String>,
}

/// 检测本机 iOS / Xcode 环境，是 services 层对外的唯一入口。
///
/// 业务失败会体现在 `status` / `message` / `issues`，而不是 `Err`。
pub fn detect() -> IosEnvironment {
    let mut issues = Vec::new();
    let mut checked_paths = Vec::new();
    let mut fatal = false;

    let developer_dir = xcode_select(&mut checked_paths);
    // 只在 xcode-select 没有配置时才回退到默认 Xcode.app；如果它明确指向
    // Command Line Tools，就不应该声称完整 Xcode 可用。
    let xcode_path = match developer_dir.as_deref() {
        Some(dir) => xcode_app_path(dir),
        None => default_xcode_path(),
    };
    if let Some(path) = &xcode_path {
        push_checked_path(&mut checked_paths, path.to_string_lossy().into_owned());
    }

    let command_line_tools_path = command_line_tools_path();
    // 无论是否存在都记录，未安装时用户才能看到完整排查范围。
    push_checked_path(&mut checked_paths, DEFAULT_XCODE_APP);
    push_checked_path(&mut checked_paths, COMMAND_LINE_TOOLS);

    if developer_dir.is_none() && command_line_tools_path.is_none() && xcode_path.is_none() {
        return not_found_environment(checked_paths);
    }

    // 只有 Command Line Tools 时，xcodebuild 会报「需要完整 Xcode」；
    // 这不是命令异常，而是明确的缺项，应该走 warning 而不是 error。
    let developer_is_command_line_tools = developer_dir
        .as_deref()
        .is_some_and(|dir| dir.trim_end_matches('/') == COMMAND_LINE_TOOLS);

    let mut xcode_version = None;
    let mut xcode_build = None;
    let mut license_accepted = None;

    if developer_dir.is_some() && !developer_is_command_line_tools {
        match run_xcodebuild_version() {
            Ok(output) if output.success() => {
                match parse_xcodebuild_version(&output.stdout) {
                    Some((version, build)) => {
                        xcode_version = Some(version);
                        xcode_build = build;
                    }
                    None => {
                        fatal = true;
                        issues.push("xcodebuild -version 输出无法解析。".to_string());
                    }
                }
                license_accepted = Some(true);
            }
            Ok(output) => {
                let text = format!("{}\n{}", output.stdout, output.stderr);
                let excerpt = output.stderr_excerpt(MESSAGE_EXCERPT_CHARS);

                if looks_like_license_error(&text) {
                    fatal = true;
                    license_accepted = Some(false);
                    issues.push(
                        "Xcode 许可协议尚未同意；请在终端运行 `sudo xcodebuild -license accept` 后再检测。".to_string(),
                    );
                } else {
                    fatal = true;
                    issues.push(if excerpt.is_empty() {
                        "xcodebuild -version 执行失败，没有输出。".to_string()
                    } else {
                        format!("xcodebuild -version 执行失败：{excerpt}")
                    });
                }
            }
            Err(error) => {
                fatal = true;
                issues.push(format!("无法执行 xcodebuild -version：{error}。"));
            }
        }
    } else if xcode_path.is_some() {
        issues.push("已找到 Xcode.app，但 xcode-select 没有指向它；命令行工具可能不可用。".to_string());
    }

    if developer_is_command_line_tools || (xcode_path.is_none() && command_line_tools_path.is_some()) {
        issues.push("只检测到 Command Line Tools，没有完整的 Xcode；无法构建 iOS 应用。".to_string());
    }

    let pod_discovery = discover_pod();
    checked_paths.extend(pod_discovery.checked_paths);
    let cocoapods = match pod_discovery.candidate {
        Some(candidate) => {
            let path = candidate.path.to_string_lossy().into_owned();
            let version = run_pod_version(&candidate.path);

            if version.is_none() {
                issues.push(format!("已找到 CocoaPods（{path}），但无法读取版本。"));
            }

            Some(CocoaPodsInfo {
                path,
                version,
                source: candidate.source,
            })
        }
        None => {
            issues.push("未安装 CocoaPods；Flutter 的 iOS 插件通常需要它。".to_string());
            None
        }
    };

    // 没有完整 Xcode 时 simctl 没有意义，也可能直接报错；不要制造额外噪声。
    let simulators = if developer_dir.is_some() && !developer_is_command_line_tools {
        read_simulators(&mut issues, license_accepted)
    } else {
        None
    };

    let swift_version = run_swift_version();
    if swift_version.is_none() {
        issues.push("未找到可用的 Swift 编译器。".to_string());
    }

    let status = overall_status(fatal, &issues);
    let message = match status {
        IosStatus::Error => Some(issues.first().cloned().unwrap_or_else(|| "iOS 环境检测失败。".to_string())),
        _ => None,
    };

    IosEnvironment {
        status,
        developer_dir,
        xcode_path: xcode_path.map(|path| path.to_string_lossy().into_owned()),
        xcode_version,
        xcode_build,
        command_line_tools_path,
        license_accepted,
        cocoapods,
        simulators,
        swift_version,
        issues,
        message,
        checked_paths,
    }
}

/// 根据致命错误和问题数量决定整体状态。
fn overall_status(fatal: bool, issues: &[String]) -> IosStatus {
    if fatal {
        IosStatus::Error
    } else if issues.is_empty() {
        IosStatus::Ok
    } else {
        IosStatus::Warning
    }
}

/// 执行 `xcode-select -p`；失败时返回 None，让调用方继续检查 CLT 和 Xcode.app。
fn xcode_select(checked_paths: &mut Vec<String>) -> Option<String> {
    let output = run_command(Path::new(XCODE_SELECT), &["-p"], Duration::from_secs(5)).ok()?;
    if !output.success() {
        return None;
    }

    let path = output.stdout.lines().map(str::trim).find(|line| !line.is_empty())?;

    if !Path::new(path).is_absolute() {
        return None;
    }

    push_checked_path(checked_paths, path.to_string());
    Some(path.to_string())
}

/// 从 `/Applications/Xcode.app/Contents/Developer` 这样的路径中提取 `.app`。
fn xcode_app_path(developer_dir: &str) -> Option<PathBuf> {
    Path::new(developer_dir)
        .ancestors()
        .find(|path| path.extension() == Some(OsStr::new("app")))
        .map(Path::to_path_buf)
}

/// 检查默认的 Xcode 安装位置。
fn default_xcode_path() -> Option<PathBuf> {
    let path = PathBuf::from(DEFAULT_XCODE_APP);
    path.is_dir().then_some(path)
}

/// 检查系统级 Command Line Tools。
fn command_line_tools_path() -> Option<String> {
    Path::new(COMMAND_LINE_TOOLS).is_dir().then(|| COMMAND_LINE_TOOLS.to_string())
}

/// 运行 `xcodebuild -version`。
fn run_xcodebuild_version() -> Result<ProcessOutput, ProcessError> {
    run_command(Path::new(XCODEBUILD), &["-version"], XCODEBUILD_TIMEOUT)
}

/// 解析 `xcodebuild -version` 的两行输出。
fn parse_xcodebuild_version(raw: &str) -> Option<(String, Option<String>)> {
    let mut version = None;
    let mut build = None;

    for line in raw.lines() {
        let line = line.trim();

        if let Some(rest) = line.strip_prefix("Xcode ")
            && let Some(token) = rest.split_whitespace().next()
            && !token.is_empty()
        {
            version = Some(token.to_string());
        } else if let Some(rest) = line.strip_prefix("Build version ") {
            let build_text = rest.trim();
            if !build_text.is_empty() {
                build = Some(build_text.to_string());
            }
        }
    }

    version.map(|version| (version, build))
}

/// 判断 xcodebuild 的输出是否在提示许可协议未接受。
fn looks_like_license_error(text: &str) -> bool {
    let lower = text.to_lowercase();

    lower.contains("license") || lower.contains("agreeing to")
}

/// 读取 iOS 模拟器列表；失败时把原因写进 issues，并返回 None。
fn read_simulators(issues: &mut Vec<String>, license_accepted: Option<bool>) -> Option<SimulatorInfo> {
    let mut command = Command::new(XCRUN);
    command.args(["simctl", "list", "devices", "available", "--json"]);

    match process::run_with_timeout(&mut command, SIMCTL_TIMEOUT) {
        Ok(output) if output.success() => match parse_simctl_json(&output.stdout) {
            Some(info) => {
                if info.available_count == 0 {
                    issues.push("没有可用的 iOS 模拟器；可以在 Xcode 的 Devices 中创建。".to_string());
                }
                Some(info)
            }
            None => {
                if license_accepted != Some(false) {
                    issues.push("无法解析 xcrun simctl 输出的模拟器列表。".to_string());
                }
                None
            }
        },
        Ok(output) => {
            if license_accepted != Some(false) {
                let excerpt = output.stderr_excerpt(MESSAGE_EXCERPT_CHARS);
                issues.push(if excerpt.is_empty() {
                    "xcrun simctl 读取模拟器列表失败。".to_string()
                } else {
                    format!("xcrun simctl 读取模拟器列表失败：{excerpt}")
                });
            }
            None
        }
        Err(error) => {
            if license_accepted != Some(false) {
                issues.push(format!("无法执行 xcrun simctl：{error}。"));
            }
            None
        }
    }
}

/// `simctl list --json` 的结构；只关心设备是否可用。
#[derive(Debug, Deserialize)]
struct SimctlRoot {
    #[serde(default)]
    devices: BTreeMap<String, Vec<SimctlDevice>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SimctlDevice {
    #[serde(default)]
    is_available: Option<bool>,
}

/// 解析 simctl JSON，只统计 iOS runtime 下的可用设备。
fn parse_simctl_json(raw: &str) -> Option<SimulatorInfo> {
    let root: SimctlRoot = serde_json::from_str(raw).ok()?;
    let mut available_count = 0;
    let mut runtimes = Vec::new();

    for (runtime, devices) in root.devices {
        if !runtime.contains(".iOS-") {
            continue;
        }

        let available = devices.iter().filter(|device| device.is_available.unwrap_or(true)).count();

        if available > 0 {
            available_count += available;
            runtimes.push(runtime_label(&runtime));
        }
    }

    runtimes.sort_by(|a, b| compare_versions(b, a));

    Some(SimulatorInfo {
        available_count,
        runtimes,
    })
}

/// 把 `com.apple.CoreSimulator.SimRuntime.iOS-27-0` 转成 `iOS 27.0`。
fn runtime_label(runtime: &str) -> String {
    match runtime.rsplit_once(".iOS-") {
        Some((_, version)) => format!("iOS {}", version.replace('-', ".")),
        None => runtime.to_string(),
    }
}

/// 运行 `swift --version` 并解析 Apple Swift 版本。
fn run_swift_version() -> Option<String> {
    let output = run_command(Path::new(SWIFT), &["--version"], SWIFT_TIMEOUT).ok()?;
    if !output.success() {
        return None;
    }

    let text = if output.stdout.trim().is_empty() {
        &output.stderr
    } else {
        &output.stdout
    };

    parse_swift_version(text)
}

/// 从 `swift --version` 输出中提取 Apple Swift 版本。
fn parse_swift_version(raw: &str) -> Option<String> {
    let marker = "Apple Swift version ";
    let start = raw.find(marker)? + marker.len();
    let version = raw[start..].split_whitespace().next()?;

    (!version.is_empty()).then(|| version.to_string())
}

/// 查找 `pod`：PATH → 常见目录 → 登录 shell。
fn discover_pod() -> PodDiscovery {
    let candidates = pod_candidates(home_dir().as_deref());
    let mut checked_paths = Vec::new();
    for candidate in &candidates {
        push_checked_path(&mut checked_paths, candidate.path.to_string_lossy().into_owned());
    }

    let mut candidate = candidates.iter().find(|candidate| is_executable(&candidate.path)).cloned();

    if candidate.is_none() {
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

            let Some(path) = parse_login_shell_pod_path(&output.stdout) else {
                continue;
            };

            push_checked_path(&mut checked_paths, path.to_string_lossy().into_owned());
            if is_executable(&path) {
                candidate = Some(PodCandidate {
                    path,
                    source: CocoaPodsSource::LoginShell,
                });
                break;
            }
        }
    }

    // 检查过的路径已经按候选顺序记录，保留给前端排查。
    PodDiscovery {
        candidate,
        checked_paths,
    }
}

/// 从登录 shell 的 `command -v pod` 输出中提取绝对路径。
fn parse_login_shell_pod_path(stdout: &str) -> Option<PathBuf> {
    stdout.lines().rev().find_map(|line| {
        let path = Path::new(line.trim());
        (path.is_absolute() && path.file_name() == Some(OsStr::new("pod"))).then(|| path.to_path_buf())
    })
}

/// 收集 `pod` 候选路径：PATH、RubyGems、rbenv/rvm、Homebrew。
fn pod_candidates(home: Option<&Path>) -> Vec<PodCandidate> {
    let mut candidates = path_pod_candidates(env::var_os("PATH").as_deref());

    if let Some(home) = home {
        for gem_root in [home.join(".gem/ruby"), home.join(".local/share/gem/ruby")] {
            for version in gem_version_dirs(&gem_root) {
                push_pod_candidate(&mut candidates, version.join("bin/pod"), CocoaPodsSource::CommonLocation);
            }
        }

        push_pod_candidate(&mut candidates, home.join(".rbenv/shims/pod"), CocoaPodsSource::CommonLocation);
        push_pod_candidate(&mut candidates, home.join(".rvm/bin/pod"), CocoaPodsSource::CommonLocation);
    }

    push_pod_candidate(&mut candidates, PathBuf::from("/opt/homebrew/bin/pod"), CocoaPodsSource::CommonLocation);
    push_pod_candidate(&mut candidates, PathBuf::from("/usr/local/bin/pod"), CocoaPodsSource::CommonLocation);

    candidates
}

/// 解析 PATH 中的 `pod` 候选，只接受绝对目录。
fn path_pod_candidates(path_var: Option<&OsStr>) -> Vec<PodCandidate> {
    let Some(path_var) = path_var else {
        return Vec::new();
    };

    let mut candidates = Vec::new();
    for directory in env::split_paths(path_var) {
        if directory.is_absolute() {
            push_pod_candidate(&mut candidates, directory.join("pod"), CocoaPodsSource::Path);
        }
    }

    candidates
}

/// RubyGems 的版本目录，例如 `~/.gem/ruby/3.3.0/bin/pod`。
fn gem_version_dirs(root: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };

    let mut versions: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    versions.sort();

    versions
}

/// 按路径去重后追加 pod 候选，保留第一次出现的来源。
fn push_pod_candidate(candidates: &mut Vec<PodCandidate>, path: PathBuf, source: CocoaPodsSource) {
    if candidates.iter().any(|existing| existing.path == path) {
        return;
    }
    candidates.push(PodCandidate { path, source });
}

/// 运行 `pod --version`。
fn run_pod_version(path: &Path) -> Option<String> {
    let output = run_command(path, &["--version"], POD_VERSION_TIMEOUT).ok()?;
    if !output.success() {
        return None;
    }

    parse_pod_version(&output.stdout).or_else(|| parse_pod_version(&output.stderr))
}

/// 解析 `pod --version` 输出，跳过 Ruby 的 warning 行。
fn parse_pod_version(raw: &str) -> Option<String> {
    raw.lines().find_map(|line| {
        let line = line.trim();

        if let Some(rest) = line.strip_prefix("CocoaPods ") {
            let version = rest.split_whitespace().next()?;
            return (!version.is_empty()).then(|| version.to_string());
        }

        if line.chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
            return line.split_whitespace().next().map(str::to_string);
        }

        None
    })
}

/// 执行固定路径的命令并带上超时。
fn run_command(path: &Path, args: &[&str], timeout: Duration) -> Result<ProcessOutput, ProcessError> {
    let mut command = Command::new(path);
    command.args(args);
    process::run_with_timeout(&mut command, timeout)
}

/// 记录一个检查过的路径，避免同一路径重复出现在排查列表里。
fn push_checked_path(paths: &mut Vec<String>, path: impl Into<String>) {
    let path = path.into();
    if !paths.iter().any(|existing| existing == &path) {
        paths.push(path);
    }
}

/// 构造「未安装」结果。
fn not_found_environment(checked_paths: Vec<String>) -> IosEnvironment {
    IosEnvironment {
        status: IosStatus::NotFound,
        developer_dir: None,
        xcode_path: None,
        xcode_version: None,
        xcode_build: None,
        command_line_tools_path: None,
        license_accepted: None,
        cocoapods: None,
        simulators: None,
        swift_version: None,
        issues: Vec::new(),
        message: Some(
            "未检测到 Xcode 或 Command Line Tools。请从 App Store 安装 Xcode，或运行 `xcode-select --install`。"
                .to_string(),
        ),
        checked_paths,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deduplicates_checked_paths() {
        let mut paths = Vec::new();

        push_checked_path(&mut paths, "/Applications/Xcode.app");
        push_checked_path(&mut paths, "/Applications/Xcode.app");
        push_checked_path(&mut paths, COMMAND_LINE_TOOLS);

        assert_eq!(paths, vec!["/Applications/Xcode.app".to_string(), COMMAND_LINE_TOOLS.to_string()]);
    }

    #[test]
    fn parses_xcodebuild_version_and_build() {
        let raw = "Xcode 27.0\nBuild version 27A266a\n";

        assert_eq!(parse_xcodebuild_version(raw), Some(("27.0".to_string(), Some("27A266a".to_string()))));
    }

    #[test]
    fn rejects_unrecognized_xcodebuild_output() {
        assert_eq!(parse_xcodebuild_version("not an xcode build\n"), None);
    }

    #[test]
    fn derives_xcode_app_only_for_app_bundles() {
        assert_eq!(
            xcode_app_path("/Applications/Xcode.app/Contents/Developer"),
            Some(PathBuf::from("/Applications/Xcode.app"))
        );
        assert_eq!(xcode_app_path(COMMAND_LINE_TOOLS), None);
    }

    #[test]
    fn parses_swift_version() {
        let raw = "swift-driver version: 1.168.6 Apple Swift version 6.4 (swiftlang-6.4.0.34.1)\nTarget: arm64-apple-macosx27.0.0\n";

        assert_eq!(parse_swift_version(raw), Some("6.4".to_string()));
    }

    #[test]
    fn detects_license_prompt() {
        let raw = "Agreeing to the Xcode/iOS license requires admin privileges, please run sudo xcodebuild -license.";

        assert!(looks_like_license_error(raw));
        assert!(!looks_like_license_error("xcodebuild: error: unknown option"));
    }

    #[test]
    fn parses_simctl_json_and_filters_ios_runtimes() {
        let raw = r#"{
            "devices": {
                "com.apple.CoreSimulator.SimRuntime.iOS-27-0": [
                    {"name": "iPhone 18 Pro", "isAvailable": true},
                    {"name": "iPhone 17", "isAvailable": false}
                ],
                "com.apple.CoreSimulator.SimRuntime.watchOS-27-0": [
                    {"name": "Apple Watch", "isAvailable": true}
                ]
            }
        }"#;

        let info = parse_simctl_json(raw).unwrap();

        assert_eq!(info.available_count, 1);
        assert_eq!(info.runtimes, vec!["iOS 27.0"]);
    }

    #[test]
    fn parses_empty_simctl_device_list() {
        let info = parse_simctl_json(r#"{"devices": {}}"#).unwrap();

        assert_eq!(info.available_count, 0);
        assert!(info.runtimes.is_empty());
    }

    #[test]
    fn parses_pod_version_and_skips_warnings() {
        assert_eq!(parse_pod_version("1.16.2\n"), Some("1.16.2".to_string()));
        assert_eq!(parse_pod_version("warning: already initialized constant\n1.16.2\n"), Some("1.16.2".to_string()));
        assert_eq!(parse_pod_version("no version here"), None);
    }

    #[test]
    fn parses_login_shell_pod_path() {
        let stdout = "Welcome to zsh\n/Users/me/.rbenv/shims/pod\n";

        assert_eq!(parse_login_shell_pod_path(stdout), Some(PathBuf::from("/Users/me/.rbenv/shims/pod")));
    }

    #[test]
    fn formats_simulator_runtime_label() {
        assert_eq!(runtime_label("com.apple.CoreSimulator.SimRuntime.iOS-27-0"), "iOS 27.0");
    }

    #[test]
    fn overall_status_prioritizes_fatal_errors() {
        assert_eq!(overall_status(true, &["license".to_string()]), IosStatus::Error);
        assert_eq!(overall_status(false, &[]), IosStatus::Ok);
        assert_eq!(overall_status(false, &["CocoaPods".to_string()]), IosStatus::Warning);
    }

    #[test]
    fn serializes_contract_in_camel_case() {
        let environment = IosEnvironment {
            status: IosStatus::Warning,
            developer_dir: Some("/Applications/Xcode.app/Contents/Developer".to_string()),
            xcode_path: Some("/Applications/Xcode.app".to_string()),
            xcode_version: Some("27.0".to_string()),
            xcode_build: Some("27A266a".to_string()),
            command_line_tools_path: Some(COMMAND_LINE_TOOLS.to_string()),
            license_accepted: Some(true),
            cocoapods: None,
            simulators: Some(SimulatorInfo {
                available_count: 2,
                runtimes: vec!["iOS 27.0".to_string()],
            }),
            swift_version: Some("6.4".to_string()),
            issues: vec!["未安装 CocoaPods".to_string()],
            message: None,
            checked_paths: Vec::new(),
        };

        let json = serde_json::to_value(&environment).unwrap();

        assert_eq!(json["status"], "warning");
        assert_eq!(json["developerDir"], "/Applications/Xcode.app/Contents/Developer");
        assert_eq!(json["licenseAccepted"], true);
        assert_eq!(json["simulators"]["availableCount"], 2);
    }
}
