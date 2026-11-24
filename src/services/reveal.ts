/**
 * 在 Finder 中定位文件。
 *
 * 使用 Tauri 官方 opener 插件；`opener:default` 已包含 reveal 权限。
 * 失败时统一抛出可读错误，页面只负责展示。
 */

import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
import { toUserMessage } from "./errors";

export async function revealInFinder(path: string): Promise<void> {
  try {
    await revealItemInDir(path);
  } catch (error) {
    throw new Error(toUserMessage(error, "无法在 Finder 中显示该文件。"));
  }
}

/** 用系统默认浏览器打开外部链接。 */
export async function openExternalUrl(url: string): Promise<void> {
  try {
    await openUrl(url);
  } catch (error) {
    throw new Error(toUserMessage(error, "无法打开链接。"));
  }
}
