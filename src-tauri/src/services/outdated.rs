//! 通过 `flutter pub outdated --json` 检查依赖更新。
//!
//! 这是显式的联网操作：只有用户点击后才执行，不在启动路径上。命令在项目目录
//! 中运行，带超时和输出上限；只读取 pub.dev 元数据，不修改 pubspec.yaml。

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use serde::Deserialize;

use crate::models::outdated::{OutdatedPackage, OutdatedReport, OutdatedStatus};
use crate::services::flutter;
use crate::utils::process::{self, MESSAGE_EXCERPT_CHARS};

/// 联网解析依赖可能较慢，给 60 秒。
const OUTDATED_TIMEOUT: Duration = Duration::from_secs(60);

/// 检查项目依赖是否可以更新。
pub fn check(project_path: &Path) -> OutdatedReport {
    if project_path.as_os_str().is_empty() {
        return failed("没有提供项目目录。".to_string());
    }
    if !project_path.join("pubspec.yaml").is_file() {
        return failed("该目录下没有 pubspec.yaml，可能不是 Flutter 项目。".to_string());
    }

    let Some(flutter) = flutter::executable_path() else {
        return failed("未检测到 Flutter SDK，无法运行 flutter pub outdated。".to_string());
    };

    let mut command = Command::new(flutter);
    command
        .args(["--no-version-check", "pub", "outdated", "--json"])
        .current_dir(project_path)
        .env(flutter::FLUTTER_ANALYTICS_ENV, "true");

    match process::run_with_timeout(&mut command, OUTDATED_TIMEOUT) {
        Ok(output) => build_report(&output),
        Err(error) => failed(format!("运行 flutter pub outdated 失败：{error}")),
    }
}

fn build_report(output: &process::ProcessOutput) -> OutdatedReport {
    let Some(json) = extract_json_object(&output.stdout).or_else(|| extract_json_object(&output.stderr)) else {
        let code = output.exit_description();
        let excerpt = output.stderr_excerpt(MESSAGE_EXCERPT_CHARS);

        return failed(if excerpt.is_empty() {
            format!("flutter pub outdated 没有输出 JSON（{code}）。")
        } else {
            format!("flutter pub outdated 失败（{code}）：{excerpt}")
        });
    };

    match parse_packages(json) {
        Ok(packages) => {
            let status = if packages.is_empty() {
                OutdatedStatus::Ok
            } else {
                OutdatedStatus::Warning
            };
            let message = if output.success() {
                None
            } else {
                let code = output.exit_description();
                Some(format!("命令以{code}结束，但已解析到部分依赖信息。"))
            };

            OutdatedReport {
                status,
                packages,
                message,
            }
        }
        Err(error) => failed(format!("无法解析 flutter pub outdated 输出：{error}")),
    }
}

/// 提取 stdout 中第一个 `{` 到最后一个 `}`，容忍前面的进度提示。
fn extract_json_object(raw: &str) -> Option<&str> {
    let start = raw.find('{')?;
    let end = raw.rfind('}')?;

    (start < end).then(|| &raw[start..=end])
}

#[derive(Debug, Deserialize)]
struct RawReport {
    #[serde(default)]
    packages: Vec<RawPackage>,
}

#[derive(Debug, Deserialize)]
struct RawPackage {
    package: String,
    #[serde(default)]
    current: Option<RawVersion>,
    #[serde(default)]
    upgradable: Option<RawVersion>,
    #[serde(default)]
    resolvable: Option<RawVersion>,
    #[serde(default)]
    latest: Option<RawVersion>,
}

#[derive(Debug, Deserialize)]
struct RawVersion {
    #[serde(default)]
    version: Option<String>,
}

fn parse_packages(json: &str) -> Result<Vec<OutdatedPackage>, serde_json::Error> {
    let report: RawReport = serde_json::from_str(json)?;
    let mut packages = report
        .packages
        .into_iter()
        .map(|package| OutdatedPackage {
            name: package.package,
            current: version_of(package.current),
            upgradable: version_of(package.upgradable),
            resolvable: version_of(package.resolvable),
            latest: version_of(package.latest),
        })
        .collect::<Vec<_>>();
    packages.sort_by(|left, right| left.name.cmp(&right.name));

    Ok(packages)
}

fn version_of(version: Option<RawVersion>) -> Option<String> {
    version.and_then(|version| version.version)
}

fn failed(message: String) -> OutdatedReport {
    OutdatedReport {
        status: OutdatedStatus::Error,
        packages: Vec::new(),
        message: Some(message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;

    #[test]
    fn parses_packages_sorted_by_name() {
        let json = r#"{
            "packages": [
                {
                    "package": "http",
                    "current": {"version": "1.0.0"},
                    "upgradable": {"version": "1.1.0"},
                    "resolvable": {"version": "1.2.0"},
                    "latest": {"version": "2.0.0"}
                },
                {
                    "package": "collection",
                    "current": {"version": "1.18.0"},
                    "latest": {"version": "1.19.0"}
                }
            ]
        }"#;

        let packages = parse_packages(json).unwrap();

        assert_eq!(packages.len(), 2);
        assert_eq!(packages[0].name, "collection");
        assert_eq!(packages[0].current.as_deref(), Some("1.18.0"));
        assert_eq!(packages[1].latest.as_deref(), Some("2.0.0"));
    }

    #[test]
    fn rejects_invalid_json() {
        assert!(parse_packages("not json").is_err());
    }

    #[test]
    fn reports_missing_pubspec_without_running_flutter() {
        let dir = env::temp_dir().join(format!("devscope-outdated-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let report = check(&dir);
        let _ = fs::remove_dir_all(&dir);

        assert_eq!(report.status, OutdatedStatus::Error);
        assert!(report.message.unwrap_or_default().contains("pubspec.yaml"), "应该说明缺少 pubspec");
    }

    #[test]
    fn extracts_json_after_progress_output() {
        let raw = "Resolving dependencies...\n{\"packages\": []}\nGot dependencies!\n";

        assert_eq!(extract_json_object(raw), Some("{\"packages\": []}"));
    }
}
