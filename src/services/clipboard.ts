/**
 * 复制文本到系统剪贴板。
 *
 * 优先使用异步 Clipboard API；Tauri 的 WebView 在部分环境下不可用时，
 * 退回 `document.execCommand("copy")`。两条路都失败才抛错，由 UI 展示。
 */

import { toUserMessage } from "./errors";

export async function copyText(text: string): Promise<void> {
  if (navigator.clipboard?.writeText) {
    try {
      await navigator.clipboard.writeText(text);
      return;
    } catch {
      // 继续尝试旧接口。
    }
  }

  if (legacyCopy(text)) {
    return;
  }

  throw new Error(toUserMessage(null, "复制失败，请手动选择文本。"));
}

function legacyCopy(text: string): boolean {
  const textarea = document.createElement("textarea");
  textarea.value = text;
  textarea.setAttribute("readonly", "");
  textarea.style.position = "fixed";
  textarea.style.top = "-1000px";
  textarea.style.opacity = "0";
  document.body.appendChild(textarea);

  try {
    textarea.select();
    return document.execCommand("copy");
  } catch {
    return false;
  } finally {
    textarea.remove();
  }
}
