/** 工具链安装引导。只展示可复制的命令和下载页，绝不自动执行。 */

import type { ToolchainKind } from "../types";

export interface InstallGuide {
  /** 推荐的安装命令，由用户复制后自行执行。 */
  command: string;
  /** 官方下载页或项目主页。 */
  url: string;
}

export const INSTALL_GUIDES: Partial<Record<ToolchainKind, InstallGuide>> = {
  python: {
    command: "brew install python",
    url: "https://www.python.org/downloads/",
  },
  node: {
    command: "brew install node",
    url: "https://nodejs.org/",
  },
  git: {
    command: "xcode-select --install",
    url: "https://git-scm.com/download/mac",
  },
  rust: {
    command: "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh",
    url: "https://rustup.rs/",
  },
  go: {
    command: "brew install go",
    url: "https://go.dev/dl/",
  },
  java: {
    command: "brew install openjdk@21",
    url: "https://adoptium.net/",
  },
};
