/**
 * 把未知错误收敛成可读字符串。
 *
 * Tauri 的 `invoke` 在 Rust 命令返回 `Err(String)` 时会 reject 一个字符串；
 * 其他情况可能是 Error 或完全未知的值。页面和 service 都通过这里转换，
 * 避免每个组件各写一份类型判断。
 */
export function toUserMessage(error: unknown, fallback = "操作失败，请查看应用日志。"): string {
  if (error instanceof Error && error.message.trim()) {
    return error.message;
  }
  if (typeof error === "string" && error.trim()) {
    return error;
  }

  return fallback;
}
