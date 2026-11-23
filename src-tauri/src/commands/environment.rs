//! 环境检测相关的 Tauri 命令。
//!
//! 这一层只做两件事：把阻塞的检测任务丢到 blocking 线程池，以及把机制性失败
//! 转成前端能读的字符串。真正的检测逻辑在 `services::flutter`。

use crate::models::environment::FlutterEnvironment;
use crate::services::flutter;

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
