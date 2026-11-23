//! 本机配置的数据契约。
//!
//! 总览页用它回答「这是一台什么机器」，与工具链检测分开：即使 Flutter 没装，
//! 本机配置也应该能展示出来。

use serde::Serialize;

/// 一台 macOS 主机的基础配置。
///
/// 每个字段都是 `Option`：某条系统命令失败时只丢这一项，整体仍然可用。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    /// 操作系统名称，通常为 macOS。
    pub os_name: Option<String>,
    /// 系统版本，例如 27.0。
    pub os_version: Option<String>,
    /// 系统 Build 号。
    pub os_build: Option<String>,
    /// CPU 架构，例如 arm64。
    pub architecture: Option<String>,
    /// 机型标识，例如 Macmini9,1。
    pub model: Option<String>,
    /// 芯片名称，例如 Apple M1。
    pub chip: Option<String>,
    /// 逻辑 CPU 核心数。
    pub cpu_cores: Option<u32>,
    /// 物理内存字节数；前端负责格式化成 GB。
    pub memory_bytes: Option<u64>,
    /// 用户可见的电脑名称。
    pub hostname: Option<String>,
}
