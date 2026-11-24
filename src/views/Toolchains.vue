<script setup lang="ts">
/**
 * 常用工具链检测页。
 *
 * 一次命令并发检测 Python / Node.js / Git / Rust / Go / Java。页面只展示版本、
 * 路径和发现方式；未安装的工具不影响其他功能，也不代表环境异常。
 */
import { computed, onMounted, ref } from "vue";
import CopyButton from "../components/CopyButton.vue";
import PathActions from "../components/PathActions.vue";
import StatusBadge from "../components/StatusBadge.vue";
import { toUserMessage } from "../services/errors";
import { INSTALL_GUIDES, type InstallGuide } from "../services/install-guide";
import { openExternalUrl } from "../services/reveal";
import { detectToolchains } from "../services/rust-api";
import { statusTone } from "../services/status";
import type { ToolchainInfo, ToolchainKind, ToolchainSource, ToolchainStatus } from "../types";

type ViewState = "loading" | "ipcError" | "empty" | "ok";
type Tone = "ok" | "warning" | "error";

interface ToolMeta {
  monogram: string;
  hint: string;
}

interface ToolView extends ToolchainInfo {
  monogram: string;
  hint: string;
  guide: InstallGuide | null;
}

const TOOL_META: Record<ToolchainKind, ToolMeta> = {
  python: { monogram: "Py", hint: "Python 解释器 python3" },
  node: { monogram: "N", hint: "Node.js 运行时 node" },
  git: { monogram: "G", hint: "版本控制 git" },
  rust: { monogram: "R", hint: "Rust 编译器 rustc" },
  go: { monogram: "Go", hint: "Go 工具链 go" },
  java: { monogram: "J", hint: "Java 运行时 java" },
};

const SOURCE_LABELS: Record<ToolchainSource, string> = {
  path: "PATH",
  commonLocation: "常见安装目录",
  loginShell: "登录 Shell",
};

const tools = ref<ToolchainInfo[]>([]);
const loading = ref(false);
const started = ref(false);
const ipcError = ref<string | null>(null);
const installError = ref<string | null>(null);

const viewState = computed<ViewState>(() => {
  if (!started.value || loading.value) {
    return "loading";
  }
  if (ipcError.value) {
    return "ipcError";
  }
  if (tools.value.length === 0) {
    return "empty";
  }

  return "ok";
});

const toolViews = computed<ToolView[]>(() =>
  tools.value.map((tool) => ({
    ...tool,
    ...TOOL_META[tool.kind],
    guide: INSTALL_GUIDES[tool.kind] ?? null,
  })),
);

const availableCount = computed(
  () => tools.value.filter((tool) => tool.status === "ok").length,
);

const overall = computed<{ tone: Tone; label: string }>(() => {
  if (!started.value || loading.value) {
    return { tone: "warning", label: "检测中" };
  }
  if (ipcError.value) {
    return { tone: "error", label: "调用失败" };
  }

  const errors = tools.value.filter((tool) => tool.status === "error").length;
  if (errors > 0) {
    return { tone: "error", label: `${errors} 项异常` };
  }
  if (availableCount.value === 0) {
    return { tone: "warning", label: "未安装" };
  }
  if (availableCount.value < tools.value.length) {
    return { tone: "warning", label: `${availableCount.value}/${tools.value.length} 可用` };
  }

  return { tone: "ok", label: "全部可用" };
});

function toolStatusLabel(status: ToolchainStatus): string {
  if (status === "ok") {
    return "已安装";
  }
  if (status === "notFound") {
    return "未安装";
  }

  return "检测异常";
}

async function openGuide(url: string): Promise<void> {
  installError.value = null;

  try {
    await openExternalUrl(url);
  } catch (error) {
    installError.value = toUserMessage(error, "无法打开下载页。");
  }
}

async function refresh(force = false): Promise<void> {
  if (loading.value) {
    return;
  }

  loading.value = true;
  ipcError.value = null;

  try {
    tools.value = await detectToolchains(force);
  } catch (error) {
    tools.value = [];
    ipcError.value = toUserMessage(error, "检测常用工具链失败，请查看应用日志。");
  } finally {
    loading.value = false;
    started.value = true;
  }
}

onMounted(() => {
  void refresh();
});
</script>

<template>
  <section class="page">
    <header class="page-header">
      <div class="page-heading">
        <p class="eyebrow">环境检测</p>
        <h1>常用工具链</h1>
        <p class="subtitle">
          检测 Python、Node.js、Git、Rust、Go、Java 的安装路径与版本。
          只执行只读的版本命令，不会安装或修改任何工具。
        </p>
      </div>

      <button class="refresh" type="button" :disabled="loading" @click="refresh(true)">
        <span v-if="loading" class="spinner spinner--small" aria-hidden="true"></span>
        {{ loading ? "检测中…" : "重新检测" }}
      </button>
    </header>

    <div v-if="viewState === 'loading'" class="card state-card" role="status" aria-live="polite">
      <span class="spinner" aria-hidden="true"></span>
      <div>
        <h2>正在检测常用工具链…</h2>
        <p>正在查找可执行文件并读取版本，请稍候。</p>
      </div>
    </div>

    <div
      v-else-if="viewState === 'ipcError'"
      class="card state-card state-card--error"
      role="alert"
    >
      <div class="state-icon state-icon--error" aria-hidden="true">!</div>
      <div class="state-body">
        <h2>无法完成工具链检测</h2>
        <p>{{ ipcError }}</p>
        <p class="hint">
          如果你是在浏览器中直接打开页面，请改用 <code>npm run tauri dev</code> 启动桌面应用。
        </p>
        <div class="state-actions">
          <button class="refresh" type="button" @click="refresh(true)">重新检测</button>
        </div>
      </div>
    </div>

    <div v-else-if="viewState === 'empty'" class="card state-card">
      <div class="state-icon state-icon--empty" aria-hidden="true">工</div>
      <div class="state-body">
        <h2>没有工具链检测结果</h2>
        <p>命令返回了空列表，可以重新检测一次。</p>
        <div class="state-actions">
          <button class="refresh" type="button" @click="refresh(true)">重新检测</button>
        </div>
      </div>
    </div>

    <template v-else>
      <div class="card">
        <div class="card-head">
          <div>
            <h2>工具链概览</h2>
            <p class="card-subtitle">
              {{ availableCount }} / {{ tools.length }} 个可用。未安装为可选项，不影响其他页面。
            </p>
          </div>
          <StatusBadge :tone="overall.tone" :label="overall.label" />
        </div>

        <p class="note">
          查找顺序：PATH → 常见安装目录（含 pyenv / nvm / fnm / SDKMAN）→ 登录 shell。
        </p>
      </div>

      <div v-if="installError" class="callout callout--error" role="alert">
        {{ installError }}
      </div>

      <div class="tool-grid">
        <div v-for="tool in toolViews" :key="tool.kind" class="card tool-card">
          <div class="tool-head">
            <span class="tool-monogram" aria-hidden="true">{{ tool.monogram }}</span>
            <div class="tool-title">
              <h2>{{ tool.name }}</h2>
              <p class="card-subtitle">{{ tool.hint }}</p>
            </div>
            <StatusBadge
              :tone="statusTone(tool.status)"
              :label="toolStatusLabel(tool.status)"
            />
          </div>

          <div v-if="tool.status === 'ok'" class="tool-version">
            {{ tool.version ?? "版本未知" }}
          </div>
          <div
            v-else
            class="callout"
            :class="{
              'callout--warning': tool.status === 'notFound',
              'callout--error': tool.status === 'error',
            }"
            role="status"
          >
            {{ tool.message ?? "没有返回具体原因。" }}
          </div>

          <dl v-if="tool.executablePath || tool.source" class="details tool-details">
            <div v-if="tool.executablePath" class="detail-row">
              <dt>安装路径</dt>
              <dd class="mono">{{ tool.executablePath }}</dd>
            </div>
            <div v-if="tool.source" class="detail-row">
              <dt>发现方式</dt>
              <dd>{{ SOURCE_LABELS[tool.source] }}</dd>
            </div>
            <div class="detail-row">
              <dt>可执行文件</dt>
              <dd class="mono">{{ tool.binary }}</dd>
            </div>
          </dl>

          <div v-if="tool.executablePath" class="tool-actions">
            <PathActions :path="tool.executablePath" />
          </div>

          <div v-if="tool.guide && tool.status !== 'ok'" class="install-guide">
            <span class="install-title">安装引导</span>
            <div class="install-command">
              <code>{{ tool.guide.command }}</code>
              <CopyButton :text="tool.guide.command" />
            </div>
            <button class="link-button" type="button" @click="openGuide(tool.guide.url)">
              打开下载页 →
            </button>
          </div>

          <details v-if="tool.checkedPaths.length > 0" class="checked">
            <summary>查看已检查的 {{ tool.checkedPaths.length }} 个位置</summary>
            <ul>
              <li v-for="path in tool.checkedPaths" :key="path">
                <code>{{ path }}</code>
              </li>
            </ul>
          </details>
        </div>
      </div>
    </template>
  </section>
</template>

<style scoped>
.state-actions {
  margin-top: 14px;
}

.note {
  margin: 14px 0 0;
  color: var(--text-tertiary);
  font-size: 11.5px;
  line-height: 1.6;
}

.tool-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px;
  align-items: start;
}

.tool-card {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.tool-head {
  display: flex;
  align-items: flex-start;
  gap: 11px;
}

.tool-monogram {
  display: grid;
  place-items: center;
  flex-shrink: 0;
  width: 30px;
  height: 30px;
  border: 1px solid var(--border);
  border-radius: 9px;
  background: var(--surface-soft);
  color: var(--text-secondary);
  font-size: 10.5px;
  font-weight: 700;
}

.tool-title {
  min-width: 0;
  flex: 1;
}

.tool-version {
  margin-top: 16px;
  font-family: "SFMono-Regular", ui-monospace, "SF Mono", Menlo, monospace;
  font-size: 23px;
  font-weight: 700;
  letter-spacing: -0.02em;
  overflow-wrap: anywhere;
}

.tool-details {
  margin-top: 14px;
  padding-top: 2px;
  border-top: 1px solid var(--border);
}

.tool-details .detail-row:first-child {
  padding-top: 11px;
}

.tool-details .detail-row {
  grid-template-columns: 90px minmax(0, 1fr);
}

.tool-actions {
  margin-top: 10px;
}

.install-guide {
  margin-top: 12px;
  padding-top: 12px;
  border-top: 1px solid var(--border);
}

.install-title {
  color: var(--text-tertiary);
  font-size: 9.5px;
  font-weight: 700;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.install-command {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  margin-top: 7px;
}

.install-command code {
  min-width: 0;
  flex: 1;
}

.link-button {
  margin-top: 7px;
  padding: 0;
  border: 0;
  background: transparent;
  color: var(--accent);
  font-size: 10.5px;
  font-weight: 600;
  cursor: pointer;
}

.link-button:hover {
  text-decoration: underline;
}

.link-button:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 1px;
}

@media (max-width: 760px) {
  .tool-grid {
    grid-template-columns: 1fr;
  }
}
</style>
