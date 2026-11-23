//! 环境检测相关的 Tauri 命令。
//!
//! 这一层只做两件事：把阻塞的检测任务丢到 blocking 线程池，以及把机制性失败
//! 转成前端能读的字符串。真正的检测逻辑在 `services::{flutter, android, ios, system}`。

use crate::models::android::AndroidEnvironment;
use crate::models::doctor::DoctorReport;
use crate::models::environment::FlutterEnvironment;
use crate::models::ios::IosEnvironment;
use crate::models::system::SystemInfo;
use crate::services::{android, flutter, ios, system};

/// 检测本机 Flutter 环境。
///
/// `flutter --version` 会启动外部进程，可能耗时数秒，因此必须用
/// `spawn_blocking`，不能直接占用 Tauri 的异步运行时。业务失败（未安装、
/// 超时、输出异常）已经收敛在 `FlutterEnvironment` 的 status 里；这里返回
/// `Err` 只表示检测任务本身没能执行完（例如线程 join 失败）。
#[tauri::command]
pub async fn detect_flutter_environment() -> Result<FlutterEnvironment, String> {
    tauri::async_runtime::spawn_blocking(flutter::detect)
        .await
        .map_err(|err| format!("Flutter 环境检测任务执行失败：{err}"))
}

/// 运行 `flutter doctor -v` 并返回体检报告。
///
/// 这是按需触发的慢操作：doctor 会检查 Android、Xcode、设备和网络，可能耗时
/// 数十秒，所以同样放到 blocking 线程池。报告本身包含 warning/error 状态，
/// 命令返回 `Err` 只表示任务没能执行完。
#[tauri::command]
pub async fn run_flutter_doctor() -> Result<DoctorReport, String> {
    tauri::async_runtime::spawn_blocking(flutter::run_flutter_doctor)
        .await
        .map_err(|err| format!("Flutter 体检任务执行失败：{err}"))
}

/// 检测本机 Android SDK、JDK 和模拟器。
///
/// 检测以文件系统读取为主，但仍放到 blocking 线程池，保持命令层行为一致，
/// 避免将来加入外部命令时阻塞异步运行时。
#[tauri::command]
pub async fn detect_android_environment() -> Result<AndroidEnvironment, String> {
    tauri::async_runtime::spawn_blocking(android::detect)
        .await
        .map_err(|err| format!("Android 环境检测任务执行失败：{err}"))
}

/// 检测本机 Xcode、Command Line Tools、CocoaPods 和 iOS 模拟器。
///
/// 会执行 `xcodebuild`、`xcrun`、`swift` 和 `pod`，都属于可能耗时数秒的
/// 阻塞操作，必须放到 blocking 线程池。
#[tauri::command]
pub async fn detect_ios_environment() -> Result<IosEnvironment, String> {
    tauri::async_runtime::spawn_blocking(ios::detect)
        .await
        .map_err(|err| format!("iOS 环境检测任务执行失败：{err}"))
}

/// 读取本机配置（macOS、芯片、内存、核心数、机型）。
///
/// 这些命令都是本地只读操作，但仍放到 blocking 线程池，避免命令层出现
/// 一个同步阻塞的例外。
#[tauri::command]
pub async fn detect_system_info() -> Result<SystemInfo, String> {
    tauri::async_runtime::spawn_blocking(system::detect)
        .await
        .map_err(|err| format!("本机配置检测任务执行失败：{err}"))
}
