/**
 * 环境状态到展示文案/颜色的统一映射。
 *
 * Flutter / Android / iOS 的状态枚举都是 `ok | warning | notFound | error` 的
 * 子集；页面和总览共用这里的映射，避免新增状态时四处修改。
 */

import type { BuildLogStatus, ProjectStatus, ReportSummaryStatus } from "../types";

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

/** 项目分析结论的中文标签。 */
export function projectStatusLabel(status: ProjectStatus): string {
  if (status === "ok") {
    return "结构完整";
  }
  if (status === "warning") {
    return "需注意";
  }

  return "无法分析";
}

/** 构建日志结论的中文标签。 */
export function logStatusLabel(status: BuildLogStatus): string {
  if (status === "ok") {
    return "未发现错误";
  }
  if (status === "error") {
    return "无法读取";
  }

  return "发现问题";
}

/** 诊断报告总体结论的中文标签。 */
export function reportSummaryLabel(status: ReportSummaryStatus): string {
  if (status === "ok") {
    return "已就绪";
  }
  if (status === "error") {
    return "需处理";
  }

  return "需注意";
}

