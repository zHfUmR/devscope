/**
 * 展示层的数值格式化。
 *
 * 只做「数值 → 给人看的一行字」，不涉及任何检测逻辑；总览页和诊断报告页
 * 描述的是同一台机器，文案必须一致。
 */

/**
 * 把字节数格式化成 GB。
 *
 * 10 GB 以上取整（16 GB 比 15.7 GB 更像用户认知里的配置），以下保留一位
 * 小数；缺失或异常值统一显示「未知」。
 */
export function formatMemory(bytes: number | null): string {
  if (!bytes || bytes <= 0) {
    return "未知";
  }

  const gb = bytes / 1024 ** 3;
  return gb >= 10 ? `${Math.round(gb)} GB` : `${gb.toFixed(1)} GB`;
}
