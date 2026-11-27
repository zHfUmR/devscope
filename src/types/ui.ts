/**
 * 页面之间共享的展示型类型。
 *
 * 这里只放纯 UI 的形状（指标卡、详情行），不放与 Rust 模型对应的数据契约——
 * IPC 契约统一在 `types/index.ts`。
 */

/** 指标卡：一行标签 + 一行值，由 app.css 的 `.metrics` / `.metric` 排版。 */
export interface Metric {
  label: string;
  value: string;
}

/** 详情行：可选 `mono` 让路径、版本号、Revision 用等宽字体展示。 */
export interface DetailRow {
  label: string;
  value: string;
  mono?: boolean;
}
