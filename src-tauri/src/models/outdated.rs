//! `flutter pub outdated --json` 的数据契约。
//!
//! 这是显式的联网操作：用户点击后才执行，不参与启动时的环境检测。

use serde::{Deserialize, Serialize};

/// 依赖更新检查的结论。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OutdatedStatus {
    /// 命令成功，且没有可更新的依赖。
    Ok,
    /// 命令成功，存在可更新依赖。
    Warning,
    /// 项目不是 Flutter 项目、找不到 flutter 或命令失败。
    Error,
}

/// 单个依赖的版本情况。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutdatedPackage {
    pub name: String,
    /// 当前锁定 / 解析到的版本。
    pub current: Option<String>,
    /// 在不改变约束的前提下可以升级到的版本。
    pub upgradable: Option<String>,
    /// 修改约束后可以解析到的版本。
    pub resolvable: Option<String>,
    /// pub.dev 上的最新版本。
    pub latest: Option<String>,
}

/// 一次依赖更新检查的结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutdatedReport {
    pub status: OutdatedStatus,
    pub packages: Vec<OutdatedPackage>,
    /// 失败原因或命令提示。
    pub message: Option<String>,
}
