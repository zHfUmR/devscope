/**
 * 环境检测页共用的异步状态机。
 *
 * Flutter / Android / iOS / 工具链四个页面都是同一件事：挂载后调一次 IPC，
 * 然后展示 loading、结果、IPC 错误三种状态。差异只在「调哪个命令」和「怎么
 * 判定空状态」，所以把状态和请求流程收在这里，页面只留自己的 viewState 映射
 * 和模板。
 *
 * 注意：IPC 失败和业务失败是两件事。业务结论（未安装、检测异常）在返回值的
 * status 里，只有调用机制本身失败才会走到 `ipcError`。
 */

import { ref, shallowRef } from "vue";
import { toUserMessage } from "../services/errors";

/** 默认的 IPC 失败文案，页面可以按场景覆盖。 */
const DEFAULT_FALLBACK = "调用 Rust 命令失败，请查看应用日志。";

export function useDetection<T>(
  detect: (force: boolean) => Promise<T>,
  fallback = DEFAULT_FALLBACK,
) {
  /**
   * 检测结果整体替换、内部不再修改，用 shallowRef 避免 Vue 深度代理整棵
   * 结果树。
   */
  const data = shallowRef<T | null>(null);
  const loading = ref(false);
  const started = ref(false);
  const ipcError = ref<string | null>(null);

  /** 执行一次检测；请求进行中重复调用会被忽略，避免按钮连点起两次进程。 */
  async function refresh(force = false): Promise<void> {
    if (loading.value) {
      return;
    }

    loading.value = true;
    ipcError.value = null;

    try {
      data.value = await detect(force);
    } catch (error) {
      data.value = null;
      ipcError.value = toUserMessage(error, fallback);
    } finally {
      loading.value = false;
      started.value = true;
    }
  }

  return { data, loading, started, ipcError, refresh };
}
