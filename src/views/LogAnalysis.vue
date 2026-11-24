<script setup lang="ts">
/**
 * 构建日志分析页。
 *
 * 用户选择一份构建日志，Rust 逐行统计错误 / 警告、合并重复行并识别构建结果。
 * 页面只做展示和本地筛选；日志内容不会上传。
 */
import { computed, onActivated, onDeactivated, onUnmounted, ref } from "vue";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import CopyButton from "../components/CopyButton.vue";
import PathActions from "../components/PathActions.vue";
import StatusBadge from "../components/StatusBadge.vue";
import { pickFile, pickTextSavePath } from "../services/dialog";
import { toUserMessage } from "../services/errors";
import { analyzeBuildLog, saveLogExport } from "../services/rust-api";
import { statusTone } from "../services/status";
import type {
  BuildLogAnalysis,
  BuildLogStatus,
  BuildOutcome,
  LogEntry,
  LogSeverity,
} from "../types";

type ViewState = "idle" | "loading" | "ipcError" | "ok" | "error";
type LogFilter = "all" | LogSeverity;
type Tone = "ok" | "warning" | "error";

interface Metric {
  label: string;
  value: string;
}

const FILTERS: Array<{ value: LogFilter; label: string }> = [
  { value: "all", label: "全部" },
  { value: "error", label: "错误" },
  { value: "warning", label: "警告" },
];

const selectedPath = ref<string | null>(null);
const analysis = ref<BuildLogAnalysis | null>(null);
const loading = ref(false);
const ipcError = ref<string | null>(null);
const filter = ref<LogFilter>("all");
const dragging = ref(false);
const exporting = ref(false);
const exportMessage = ref<string | null>(null);
const exportError = ref<string | null>(null);
let unlistenDragDrop: (() => void) | null = null;

const viewState = computed<ViewState>(() => {
  if (loading.value) {
    return "loading";
  }
  if (ipcError.value) {
    return "ipcError";
  }
  if (!analysis.value) {
    return "idle";
  }

  return analysis.value.status === "error" ? "error" : "ok";
});

const metrics = computed<Metric[]>(() => {
  const data = analysis.value;
  if (!data) {
    return [];
  }

  return [
    { label: "分析行数", value: String(data.totalLines) },
    { label: "错误", value: String(data.errorCount) },
    { label: "警告", value: String(data.warningCount) },
    { label: "问题分组", value: String(data.entries.length + data.omittedEntries) },
  ];
});

const filteredEntries = computed<LogEntry[]>(() => {
  const data = analysis.value;
  if (!data) {
    return [];
  }
  if (filter.value === "all") {
    return data.entries;
  }

  return data.entries.filter((entry) => entry.severity === filter.value);
});

const hasFindings = computed(
  () => (analysis.value?.errorCount ?? 0) + (analysis.value?.warningCount ?? 0) > 0,
);

const errorMessage = computed(
  () => analysis.value?.message ?? "无法读取日志文件，请确认路径是否正确。",
);

const filteredText = computed(() => {
  const data = analysis.value;
  if (!data) {
    return "";
  }

  const header = [
    `日志：${data.path}`,
    `文件：${data.fileName}`,
    `筛选：${filter.value === "all" ? "全部" : filter.value === "error" ? "错误" : "警告"}`,
    `错误 ${data.errorCount} / 警告 ${data.warningCount}`,
    "",
  ].join("\n");

  return (
    header +
    filteredEntries.value
      .map((entry) => {
        const count = entry.count > 1 ? `（出现 ${entry.count} 次）` : "";
        return `[${entry.severity === "error" ? "错误" : "警告"}] 第 ${entry.firstLine} 行${count} ${entry.message}`;
      })
      .join("\n")
  );
});

function setFilter(value: LogFilter): void {
  filter.value = value;
}

function logStatusLabel(status: BuildLogStatus): string {
  if (status === "ok") {
    return "未发现错误";
  }
  if (status === "error") {
    return "无法读取";
  }

  return "发现问题";
}

function outcomeBadge(outcome: BuildOutcome): { tone: Tone; label: string } {
  if (outcome === "success") {
    return { tone: "ok", label: "构建成功" };
  }
  if (outcome === "failure") {
    return { tone: "error", label: "构建失败" };
  }

  return { tone: "warning", label: "构建结果未知" };
}

function severityLabel(severity: LogSeverity): string {
  return severity === "error" ? "错误" : "警告";
}

function lineRange(entry: LogEntry): string {
  if (entry.firstLine === entry.lastLine) {
    return `第 ${entry.firstLine} 行`;
  }

  return `第 ${entry.firstLine}–${entry.lastLine} 行`;
}

function formatBytes(bytes: number): string {
  if (bytes >= 1024 * 1024) {
    return `${(bytes / (1024 * 1024)).toFixed(1)} MiB`;
  }
  if (bytes >= 1024) {
    return `${(bytes / 1024).toFixed(1)} KiB`;
  }

  return `${bytes} 字节`;
}

async function registerDragDrop(): Promise<void> {
  if (unlistenDragDrop) {
    return;
  }

  try {
    unlistenDragDrop = await getCurrentWebview().onDragDropEvent((event) => {
      if (event.payload.type === "enter" || event.payload.type === "over") {
        dragging.value = true;
        return;
      }
      if (event.payload.type === "leave") {
        dragging.value = false;
        return;
      }

      dragging.value = false;
      const path = event.payload.paths[0];
      if (path) {
        selectedPath.value = path;
        void runAnalysis(path);
      }
    });
  } catch {
    // 浏览器预览或没有拖放权限时静默降级，仍可用文件选择器。
  }
}

function unregisterDragDrop(): void {
  dragging.value = false;
  unlistenDragDrop?.();
  unlistenDragDrop = null;
}

async function runAnalysis(path: string): Promise<void> {
  loading.value = true;
  ipcError.value = null;
  analysis.value = null;
  filter.value = "all";
  exportMessage.value = null;
  exportError.value = null;

  try {
    analysis.value = await analyzeBuildLog(path);
  } catch (error) {
    ipcError.value = toUserMessage(error, "分析日志失败，请查看应用日志。");
  } finally {
    loading.value = false;
  }
}

async function chooseLog(): Promise<void> {
  try {
    const path = await pickFile("选择构建日志", []);
    if (!path) {
      return;
    }

    selectedPath.value = path;
    await runAnalysis(path);
  } catch (error) {
    ipcError.value = toUserMessage(error, "无法打开文件选择器。");
  }
}

async function exportFiltered(): Promise<void> {
  if (!analysis.value || filteredEntries.value.length === 0 || exporting.value) {
    return;
  }

  exporting.value = true;
  exportError.value = null;
  exportMessage.value = null;

  try {
    const stamp = new Date()
      .toISOString()
      .slice(0, 19)
      .replace(/[:T]/g, "-");
    const path = await pickTextSavePath(
      "导出筛选结果",
      `devscope-log-${stamp}.txt`,
      "txt",
      "文本日志",
    );
    if (!path) {
      return;
    }

    await saveLogExport(path, filteredText.value);
    exportMessage.value = `已导出 ${filteredEntries.value.length} 条记录到 ${path}`;
  } catch (error) {
    exportError.value = toUserMessage(error, "导出筛选结果失败，请查看应用日志。");
  } finally {
    exporting.value = false;
  }
}

async function reanalyze(): Promise<void> {
  if (!selectedPath.value || loading.value) {
    return;
  }

  await runAnalysis(selectedPath.value);
}

onActivated(() => {
  void registerDragDrop();
});

onDeactivated(unregisterDragDrop);
onUnmounted(unregisterDragDrop);
</script>

<template>
  <section class="page">
    <div v-if="dragging" class="drop-overlay" aria-hidden="true">
      <div class="drop-card">松开以分析日志</div>
    </div>

    <header class="page-header">
      <div class="page-heading">
        <p class="eyebrow">日志分析</p>
        <h1>构建日志</h1>
        <p class="subtitle">
          选择 Gradle、Xcode 或 Flutter 的构建日志，统计错误与警告并合并重复内容。日志只在本地读取。
        </p>
      </div>

      <div class="header-actions">
        <button
          class="refresh refresh--primary"
          type="button"
          :disabled="loading"
          @click="chooseLog"
        >
          {{ analysis ? "更换日志" : "选择日志" }}
        </button>
        <button
          v-if="selectedPath"
          class="refresh"
          type="button"
          :disabled="loading"
          @click="reanalyze"
        >
          <span v-if="loading" class="spinner spinner--small" aria-hidden="true"></span>
          {{ loading ? "分析中…" : "重新分析" }}
        </button>
      </div>
    </header>

    <div v-if="selectedPath" class="path-bar">
      <span class="path-label">当前文件</span>
      <code>{{ selectedPath }}</code>
      <PathActions :path="selectedPath" />
    </div>

    <div v-if="viewState === 'loading'" class="card state-card" role="status" aria-live="polite">
      <span class="spinner" aria-hidden="true"></span>
      <div>
        <h2>正在分析日志…</h2>
        <p>逐行识别错误、警告和构建结果。</p>
      </div>
    </div>

    <div
      v-else-if="viewState === 'ipcError'"
      class="card state-card state-card--error"
      role="alert"
    >
      <div class="state-icon state-icon--error" aria-hidden="true">!</div>
      <div class="state-body">
        <h2>无法完成日志分析</h2>
        <p>{{ ipcError }}</p>
        <p class="hint">
          如果你是在浏览器中直接打开页面，请改用 <code>npm run tauri dev</code> 启动桌面应用。
        </p>
        <div class="state-actions">
          <button class="refresh refresh--primary" type="button" @click="chooseLog">
            重新选择日志
          </button>
        </div>
      </div>
    </div>

    <div v-else-if="viewState === 'idle'" class="card state-card">
      <div class="state-icon state-icon--empty" aria-hidden="true">日</div>
      <div class="state-body">
        <h2>选择一份构建日志</h2>
        <p>
          支持 Gradle、Xcode、Dart / Flutter 等常见构建输出。分析会忽略 ANSI 颜色码，
          并把完全相同的错误行合并计数。也可以直接把日志文件拖到窗口里。
        </p>
        <div class="state-actions">
          <button class="refresh refresh--primary" type="button" @click="chooseLog">
            选择日志文件
          </button>
        </div>
      </div>
    </div>

    <div
      v-else-if="viewState === 'error'"
      class="card state-card state-card--error"
      role="alert"
    >
      <div class="state-icon state-icon--error" aria-hidden="true">!</div>
      <div class="state-body">
        <div class="state-heading">
          <h2>无法读取日志</h2>
          <StatusBadge tone="error" label="读取失败" />
        </div>
        <p>{{ errorMessage }}</p>
        <div class="state-actions">
          <button class="refresh refresh--primary" type="button" @click="chooseLog">
            重新选择日志
          </button>
        </div>
      </div>
    </div>

    <template v-else-if="analysis">
      <div class="card">
        <div class="card-head">
          <div>
            <h2>{{ analysis.fileName }}</h2>
            <p class="card-subtitle">
              {{ formatBytes(analysis.fileSizeBytes) }} ·
              {{ formatBytes(analysis.analyzedBytes) }} 已分析
            </p>
          </div>
          <div class="badge-group">
            <StatusBadge
              :tone="outcomeBadge(analysis.buildOutcome).tone"
              :label="outcomeBadge(analysis.buildOutcome).label"
            />
            <StatusBadge
              :tone="statusTone(analysis.status)"
              :label="logStatusLabel(analysis.status)"
            />
          </div>
        </div>

        <div class="metrics">
          <div v-for="metric in metrics" :key="metric.label" class="metric">
            <span class="metric-label">{{ metric.label }}</span>
            <span class="metric-value">{{ metric.value }}</span>
          </div>
        </div>

        <div v-if="analysis.message" class="callout callout--warning" role="status">
          {{ analysis.message }}
        </div>
      </div>

      <div class="card">
        <div class="card-head">
          <div>
            <h2>问题分组</h2>
            <p class="card-subtitle">相同内容合并计数，按错误优先、出现次数排序。</p>
          </div>
          <span v-if="analysis.omittedEntries > 0" class="omitted">
            另有 {{ analysis.omittedEntries }} 组未展示
          </span>
        </div>

        <div v-if="hasFindings" class="filters">
          <button
            v-for="item in FILTERS"
            :key="item.value"
            class="filter-button"
            :class="{ 'filter-button--active': filter === item.value }"
            type="button"
            @click="setFilter(item.value)"
          >
            {{ item.label }}
            <span class="filter-count">
              {{
                item.value === "all"
                  ? analysis.errorCount + analysis.warningCount
                  : item.value === "error"
                    ? analysis.errorCount
                    : analysis.warningCount
              }}
            </span>
          </button>
          <CopyButton
            v-if="filteredEntries.length > 0"
            :text="filteredText"
            label="复制筛选结果"
          />
          <button
            v-if="filteredEntries.length > 0"
            class="filter-button"
            type="button"
            :disabled="exporting"
            @click="exportFiltered"
          >
            <span v-if="exporting" class="spinner spinner--small" aria-hidden="true"></span>
            {{ exporting ? "导出中…" : "导出筛选结果" }}
          </button>
        </div>

        <div v-if="exportError" class="callout callout--error" role="alert">
          {{ exportError }}
        </div>
        <div v-else-if="exportMessage" class="callout callout--success" role="status">
          {{ exportMessage }}
        </div>

        <div v-if="!hasFindings" class="empty-note">
          未发现错误或警告。如果是空文件或日志被截断，上方会有提示。
        </div>

        <div v-else-if="filteredEntries.length === 0" class="empty-note">
          当前筛选条件下没有记录。
        </div>

        <ul v-else class="log-list">
          <li v-for="entry in filteredEntries" :key="`${entry.firstLine}-${entry.message}`" class="log-entry">
            <StatusBadge
              :tone="entry.severity === 'error' ? 'error' : 'warning'"
              :label="severityLabel(entry.severity)"
            />
            <div class="log-body">
              <p class="log-message">{{ entry.message }}</p>
              <p class="log-meta">
                {{ lineRange(entry) }}
                <span v-if="entry.count > 1"> · 出现 {{ entry.count }} 次</span>
              </p>
              <div class="log-actions">
                <CopyButton :text="entry.message" label="复制内容" />
                <CopyButton
                  :text="`${analysis.path}:${entry.firstLine}`"
                  label="复制位置"
                />
              </div>
            </div>
          </li>
        </ul>
      </div>
    </template>
  </section>
</template>

<style scoped>
.header-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.path-bar {
  display: flex;
  align-items: baseline;
  gap: 10px;
  padding: 10px 14px;
  border: 1px solid var(--border);
  border-radius: 10px;
  background: var(--surface-soft);
}

.path-label {
  flex-shrink: 0;
  color: var(--text-tertiary);
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.path-bar code {
  min-width: 0;
  flex: 1;
  padding: 0;
  background: transparent;
  color: var(--text-secondary);
  font-size: 11.5px;
}

.badge-group {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.state-actions {
  margin-top: 14px;
}

.omitted {
  flex-shrink: 0;
  color: var(--text-tertiary);
  font-size: 11px;
}

.callout--success {
  border-color: var(--ok-border);
  background: var(--ok-soft);
  color: var(--ok);
}

.filters {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-top: 16px;
}

.filter-button {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 5px 10px;
  border: 1px solid var(--border);
  border-radius: 7px;
  background: var(--surface);
  color: var(--text-secondary);
  font-size: 11.5px;
  font-weight: 600;
  cursor: pointer;
}

.filter-button:hover {
  border-color: var(--border-strong);
  background: var(--surface-hover);
}

.filter-button:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 1px;
}

.filter-button--active {
  border-color: transparent;
  background: var(--text);
  color: var(--on-accent);
}

.filter-count {
  font-size: 10px;
  opacity: 0.7;
}

.log-list {
  margin: 12px 0 0;
  padding: 0;
  list-style: none;
}

.log-entry {
  display: flex;
  align-items: flex-start;
  gap: 12px;
  padding: 12px 0;
  border-top: 1px solid var(--border);
}

.log-entry:first-child {
  border-top: 0;
  padding-top: 0;
}

.log-body {
  min-width: 0;
  flex: 1;
}

.log-message {
  margin: 0;
  color: var(--text);
  font-family: "SFMono-Regular", ui-monospace, "SF Mono", Menlo, monospace;
  font-size: 11.5px;
  line-height: 1.55;
  overflow-wrap: anywhere;
  white-space: pre-wrap;
}

.log-meta {
  margin: 4px 0 0;
  color: var(--text-tertiary);
  font-size: 10.5px;
}

.log-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-top: 7px;
}

.drop-overlay {
  position: fixed;
  inset: 0;
  z-index: 90;
  display: grid;
  place-items: center;
  padding: 24px;
  background: rgba(0, 0, 0, 0.28);
  backdrop-filter: blur(6px);
  -webkit-backdrop-filter: blur(6px);
  pointer-events: none;
}

.drop-card {
  padding: 26px 34px;
  border: 1.5px dashed var(--accent);
  border-radius: 16px;
  background: var(--surface);
  color: var(--text);
  font-size: 14px;
  font-weight: 600;
  box-shadow: var(--shadow);
}

@media (max-width: 680px) {
  .path-bar {
    align-items: flex-start;
    flex-direction: column;
    gap: 4px;
  }
}
</style>
