/**
 * 原生文件 / 目录选择对话框。
 *
 * 所有对话框调用集中在这里，页面只处理返回的路径（或 null 表示取消）。
 * 真正的文件读取与导出仍由 Rust 完成，符合「系统操作只在 Rust」的边界。
 */

import { open, save } from "@tauri-apps/plugin-dialog";
import type { ReportFormat } from "../types";

/** 选择一个目录，取消时返回 null。 */
export async function pickDirectory(title: string): Promise<string | null> {
  const selected = await open({
    title,
    directory: true,
    multiple: false,
  });

  return firstPath(selected);
}

/** 选择一个文件，取消时返回 null。 */
export async function pickFile(
  title: string,
  extensions: string[],
): Promise<string | null> {
  const selected = await open({
    title,
    multiple: false,
    ...(extensions.length > 0 ? { filters: [{ name: "文件", extensions }] } : {}),
  });

  return firstPath(selected);
}

/** 选择导出路径，取消时返回 null。 */
export async function pickSavePath(
  title: string,
  defaultPath: string,
  format: ReportFormat,
): Promise<string | null> {
  const extension = format === "json" ? "json" : "md";
  const selected = await save({
    title,
    defaultPath,
    filters: [
      {
        name: format === "json" ? "JSON 报告" : "Markdown 报告",
        extensions: [extension],
      },
    ],
  });

  return selected ?? null;
}

/** 选择任意文本文件的保存路径，取消时返回 null。 */
export async function pickTextSavePath(
  title: string,
  defaultName: string,
  extension: string,
  filterName: string,
): Promise<string | null> {
  const selected = await save({
    title,
    defaultPath: defaultName,
    filters: [{ name: filterName, extensions: [extension] }],
  });

  return selected ?? null;
}

/** `open()` 的类型取决于 multiple 参数；这里统一处理单路径场景。 */
function firstPath(selected: string | string[] | null): string | null {
  if (Array.isArray(selected)) {
    return selected[0] ?? null;
  }

  return selected;
}
