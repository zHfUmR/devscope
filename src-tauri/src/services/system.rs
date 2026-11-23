//! 本机配置检测：macOS 版本、芯片、内存、核心数和机型。
//!
//! 这里只读取系统公开的命令输出，不扫描用户目录、不修改任何设置。
//! 每条命令单独执行并带超时，某一项失败时只把该字段留空。

use std::process::Command;
use std::time::Duration;

use crate::models::system::SystemInfo;
use crate::utils::process;

/// 单条系统信息命令的超时；这些都是本地只读命令，2 秒足够。
const COMMAND_TIMEOUT: Duration = Duration::from_secs(2);

/// 检测本机配置，是 services 层对外的唯一入口。
///
/// 返回结构里的字段允许为空：即便某条命令失败，调用方仍然可以展示其余信息。
pub fn detect() -> SystemInfo {
    let sw_vers = command_stdout("/usr/bin/sw_vers", &[]);
    let (os_name, os_version, os_build) = sw_vers.as_deref().map(parse_sw_vers).unwrap_or((None, None, None));

    SystemInfo {
        os_name,
        os_version,
        os_build,
        architecture: command_first_line("/usr/bin/uname", &["-m"]),
        model: sysctl_value("hw.model"),
        chip: sysctl_value("machdep.cpu.brand_string"),
        cpu_cores: sysctl_value("hw.ncpu").and_then(|value| value.parse().ok()),
        memory_bytes: sysctl_value("hw.memsize").and_then(|value| value.parse().ok()),
        hostname: command_first_line("/usr/sbin/scutil", &["--get", "ComputerName"])
            .or_else(|| command_first_line("/bin/hostname", &[])),
    }
}

/// 解析 `sw_vers` 的 `Key: Value` 输出。
///
/// 只关心 ProductName / ProductVersion / BuildVersion，其他行忽略，
/// 这样 Apple 以后加字段也不会影响解析。
fn parse_sw_vers(raw: &str) -> (Option<String>, Option<String>, Option<String>) {
    let mut name = None;
    let mut version = None;
    let mut build = None;

    for line in raw.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        if value.is_empty() {
            continue;
        }

        match key.trim() {
            "ProductName" => name = Some(value.to_string()),
            "ProductVersion" => version = Some(value.to_string()),
            "BuildVersion" => build = Some(value.to_string()),
            _ => {}
        }
    }

    (name, version, build)
}

/// 读取 `sysctl -n <key>` 的值。
fn sysctl_value(key: &str) -> Option<String> {
    command_first_line("/usr/sbin/sysctl", &["-n", key])
}

/// 执行命令并返回第一行非空输出。
fn command_first_line(path: &str, args: &[&str]) -> Option<String> {
    command_stdout(path, args)?
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_string)
}

/// 执行命令并返回完整 stdout。
fn command_stdout(path: &str, args: &[&str]) -> Option<String> {
    let mut command = Command::new(path);
    command.args(args);

    let output = process::run_with_timeout(&mut command, COMMAND_TIMEOUT).ok()?;
    if !output.success() {
        return None;
    }

    let text = output.stdout.trim();
    (!text.is_empty()).then(|| text.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sw_vers_output() {
        let raw = "ProductName:\t\tmacOS\nProductVersion:\t\t27.0\nBuildVersion:\t\t26A428\n";

        assert_eq!(
            parse_sw_vers(raw),
            (Some("macOS".to_string()), Some("27.0".to_string()), Some("26A428".to_string()))
        );
    }

    #[test]
    fn ignores_unknown_sw_vers_lines() {
        let raw = "ProductName: macOS\nSomeOther: value\nProductVersion: 27.0\n";

        assert_eq!(parse_sw_vers(raw), (Some("macOS".to_string()), Some("27.0".to_string()), None));
    }

    #[test]
    fn handles_empty_sw_vers_output() {
        assert_eq!(parse_sw_vers(""), (None, None, None));
    }
}
