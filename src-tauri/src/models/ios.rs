//! iOS / Xcode 开发环境的数据契约。
//!
//! iOS 工具链由 Xcode、Command Line Tools、CocoaPods 和模拟器组成。
//! Xcode 的授权状态和模拟器列表都是可选项，缺任何一项都会体现在 `issues` 中。

use serde::Serialize;

/// 一次 iOS 环境检测的业务结论。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum IosStatus {
    /// Xcode、CocoaPods 和模拟器都可用。
    Ok,
    /// 找到了 Xcode，但缺少 CocoaPods、模拟器等可修复项。
    Warning,
    /// 既没有完整的 Xcode，也没有 Command Line Tools。
    NotFound,
    /// Xcode 存在，但授权未接受或命令无法执行。
    Error,
}

/// CocoaPods 可执行文件的发现方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CocoaPodsSource {
    /// 来自 GUI 进程继承的 PATH。
    Path,
    /// 来自 Homebrew、RubyGems 等常见目录。
    CommonLocation,
    /// 通过用户登录 shell 的 `command -v pod` 解析得到。
    LoginShell,
}

/// 一个可用的 CocoaPods 安装。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CocoaPodsInfo {
    /// `pod` 可执行文件的绝对路径。
    pub path: String,
    /// `pod --version` 输出的版本；执行失败时为 None。
    pub version: Option<String>,
    /// 这个可执行文件是通过哪种方式发现的。
    pub source: CocoaPodsSource,
}

/// iOS 模拟器的汇总信息。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SimulatorInfo {
    /// 可用的 iOS 模拟器数量。
    pub available_count: usize,
    /// 已安装的 iOS runtime，例如 `iOS 27.0`，按版本从新到旧排列。
    pub runtimes: Vec<String>,
}

/// 一次 iOS / Xcode 环境检测的完整结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IosEnvironment {
    /// 业务结论，而不是 IPC 层的成功/失败。
    pub status: IosStatus,
    /// `xcode-select -p` 返回的开发者目录。
    pub developer_dir: Option<String>,
    /// Xcode.app 的路径；只装了 Command Line Tools 时为 None。
    pub xcode_path: Option<String>,
    /// `xcodebuild -version` 中的 Xcode 版本。
    pub xcode_version: Option<String>,
    /// `xcodebuild -version` 中的 Build 号。
    pub xcode_build: Option<String>,
    /// `/Library/Developer/CommandLineTools`，存在时记录。
    pub command_line_tools_path: Option<String>,
    /// Xcode 许可协议是否已接受；无法判断时为 None。
    pub license_accepted: Option<bool>,
    /// 找到的 CocoaPods；None 表示未安装。
    pub cocoapods: Option<CocoaPodsInfo>,
    /// 模拟器汇总；无法读取 simctl 输出时为 None。
    pub simulators: Option<SimulatorInfo>,
    /// `swift --version` 中的 Apple Swift 版本。
    pub swift_version: Option<String>,
    /// 需要用户关注但不会中断检测的问题。
    pub issues: Vec<String>,
    /// 整体不可用时的原因；正常或只缺可选项时为 None。
    pub message: Option<String>,
    /// 本次探测过的开发者目录、Xcode 和 CocoaPods 候选路径。
    pub checked_paths: Vec<String>,
}
