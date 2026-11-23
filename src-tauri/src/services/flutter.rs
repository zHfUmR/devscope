//! Flutter 环境检测的纯逻辑。
//!
//! 当前只负责解析 `flutter --version --machine` 的输出；路径发现、进程执行和
//! Tauri 命令会在后续步骤接入。把解析拆成纯函数，测试就不依赖本机是否装了 Flutter。

use std::fmt;

use serde::Deserialize;

use crate::models::environment::FlutterVersionInfo;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::environment::{FlutterEnvironment, FlutterExecutableSource, FlutterStatus};
    use serde_json::json;

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
}
