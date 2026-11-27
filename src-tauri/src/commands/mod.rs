//! Tauri 命令层。
//!
//! 这一层只负责参数进出：把阻塞的检测任务丢进 blocking 线程池、把机制性失败
//! 转成前端能读的字符串。业务逻辑一律在 `services/`。

pub mod analysis;
pub mod environment;

/// 把可能耗时的同步任务丢到 blocking 线程池，并统一包装 join 失败。
///
/// `context` 只用于拼错误信息（例如「Flutter 环境检测任务执行失败」）。业务
/// 失败不在这里处理：各命令自己把它收敛到返回模型的 `status` / `message`，
/// 所以这里剩下的 `Err` 只表示任务本身没跑完（线程 panic 等）。
pub(crate) async fn run_blocking<T, F>(context: &str, task: F) -> Result<T, String>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    tauri::async_runtime::spawn_blocking(task)
        .await
        .map_err(|err| format!("{context}任务执行失败：{err}"))
}
