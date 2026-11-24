//! 环境检测相关的 Tauri 命令。
//!
//! 这一层只做三件事：把阻塞的检测任务丢到 blocking 线程池、通过共享缓存复用
//! 最近结果、把机制性失败转成前端能读的字符串。真正的检测逻辑在
//! `services::{flutter, android, ios, system, toolchain}`。
//!
//! `force = true` 表示用户主动刷新，绕过 30 秒 TTL；页面首次加载不传或传
//! `false`，可以直接复用总览页刚执行过的检测。

use crate::models::android::AndroidEnvironment;
use crate::models::doctor::DoctorReport;
use crate::models::environment::FlutterEnvironment;
use crate::models::ios::IosEnvironment;
use crate::models::system::SystemInfo;
use crate::models::toolchain::ToolchainInfo;
use crate::services::cache::DetectionCache;
use crate::services::flutter;

/// 检测本机 Flutter 环境。
///
/// `flutter --version` 会启动外部进程，可能耗时数秒，因此必须用
/// `spawn_blocking`，不能直接占用 Tauri 的异步运行时。业务失败（未安装、
/// 超时、输出异常）已经收敛在 `FlutterEnvironment` 的 status 里；这里返回
/// `Err` 只表示检测任务本身没能执行完（例如线程 join 失败）。
#[tauri::command]
pub async fn detect_flutter_environment(
    force: Option<bool>,
    cache: tauri::State<'_, DetectionCache>,
) -> Result<FlutterEnvironment, String> {
    let cache = cache.inner().clone();
    let force = force.unwrap_or(false);

    tauri::async_runtime::spawn_blocking(move || cache.flutter(force))
        .await
        .map_err(|err| format!("Flutter 环境检测任务执行失败：{err}"))
}

/// 运行 `flutter doctor -v` 并返回体检报告。
///
/// doctor 是按需触发的慢操作，且结果带有时间点信息，不进入 TTL 缓存。
#[tauri::command]
pub async fn run_flutter_doctor() -> Result<DoctorReport, String> {
    tauri::async_runtime::spawn_blocking(flutter::run_flutter_doctor)
        .await
        .map_err(|err| format!("Flutter 体检任务执行失败：{err}"))
}

/// 检测本机 Android SDK、JDK 和模拟器。
#[tauri::command]
pub async fn detect_android_environment(
    force: Option<bool>,
    cache: tauri::State<'_, DetectionCache>,
) -> Result<AndroidEnvironment, String> {
    let cache = cache.inner().clone();
    let force = force.unwrap_or(false);

    tauri::async_runtime::spawn_blocking(move || cache.android(force))
        .await
        .map_err(|err| format!("Android 环境检测任务执行失败：{err}"))
}

/// 检测本机 Xcode、Command Line Tools、CocoaPods 和 iOS 模拟器。
///
/// 会执行 `xcodebuild`、`xcrun`、`swift` 和 `pod`，都属于可能耗时数秒的
/// 阻塞操作，必须放到 blocking 线程池。
#[tauri::command]
pub async fn detect_ios_environment(
    force: Option<bool>,
    cache: tauri::State<'_, DetectionCache>,
) -> Result<IosEnvironment, String> {
    let cache = cache.inner().clone();
    let force = force.unwrap_or(false);

    tauri::async_runtime::spawn_blocking(move || cache.ios(force))
        .await
        .map_err(|err| format!("iOS 环境检测任务执行失败：{err}"))
}

/// 读取本机配置（macOS、芯片、内存、核心数、机型）。
#[tauri::command]
pub async fn detect_system_info(
    force: Option<bool>,
    cache: tauri::State<'_, DetectionCache>,
) -> Result<SystemInfo, String> {
    let cache = cache.inner().clone();
    let force = force.unwrap_or(false);

    tauri::async_runtime::spawn_blocking(move || cache.system(force))
        .await
        .map_err(|err| format!("本机配置检测任务执行失败：{err}"))
}

/// 检测常用工具链（Python / Node.js / Git / Rust / Go / Java）。
#[tauri::command]
pub async fn detect_toolchains(
    force: Option<bool>,
    cache: tauri::State<'_, DetectionCache>,
) -> Result<Vec<ToolchainInfo>, String> {
    let cache = cache.inner().clone();
    let force = force.unwrap_or(false);

    tauri::async_runtime::spawn_blocking(move || cache.toolchains(force))
        .await
        .map_err(|err| format!("常用工具链检测任务执行失败：{err}"))
}
