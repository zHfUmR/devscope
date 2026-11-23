/**
 * 环境状态到展示文案/颜色的统一映射。
 *
 * Flutter / Android / iOS 的状态枚举都是 `ok | warning | notFound | error` 的
 * 子集；页面和总览共用这里的映射，避免新增状态时四处修改。
 */

export type StatusTone = "ok" | "warning" | "error";

/** 把业务状态映射成 StatusBadge 的 tone。 */
export function statusTone(status: string | undefined): StatusTone {
  if (status === "ok") {
    return "ok";
  }
  if (status === "error") {
    return "error";
  }

  return "warning";
}

/** 把业务状态映射成中文标签。 */
export function statusLabel(status: string | undefined): string {
  if (status === "ok") {
    return "已就绪";
  }
  if (status === "notFound") {
    return "未安装";
  }
  if (status === "error") {
    return "检测异常";
  }

  return "需注意";
}
