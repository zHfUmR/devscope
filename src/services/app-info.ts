/** 应用元信息。版本号来自 Tauri，读取失败时退回打包版本。 */

import { getVersion } from "@tauri-apps/api/app";

export const REPOSITORY_URL = "https://github.com/zHfUmR/devscope";
export const LICENSE_URL = `${REPOSITORY_URL}/blob/main/LICENSE`;
export const ISSUES_URL = `${REPOSITORY_URL}/issues`;

export async function getAppVersion(): Promise<string> {
  try {
    return await getVersion();
  } catch {
    return "0.1.0";
  }
}
