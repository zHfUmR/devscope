<script setup lang="ts">
/**
 * 诊断报告页。
 *
 * 先生成报告（执行本机环境检测），预览摘要后导出 JSON / Markdown。
 * 报告对象由 Rust 返回，导出时原样回传，避免重复执行耗时的检测。
 */
import { computed, onMounted, ref } from "vue";
import DetailList from "../components/DetailList.vue";
import PathActions from "../components/PathActions.vue";
import StatusBadge from "../components/StatusBadge.vue";
import { copyText } from "../services/clipboard";
import { pickDirectory, pickFile, pickSavePath } from "../services/dialog";
import { toUserMessage } from "../services/errors";
import { formatMemory } from "../services/format";
import {
  generateDiagnosticReport,
  loadDiagnosticReport,
  printCurrentWindow,
  renderDiagnosticReport,
  saveDiagnosticReport,
} from "../services/rust-api";
import {
  logStatusLabel,
  projectStatusLabel,
  reportSummaryLabel,
  statusLabel,
  statusTone,
} from "../services/status";
import type { DiagnosticReport, PlatformKind, ReportFormat } from "../types";
import type { DetailRow } from "../types/ui";

interface RecentExport {
  path: string;
  format: ReportFormat;
  at: number;
}

interface DiffRow {
  label: string;
  before: string;
  after: string;
  changed: boolean;
}

const RECENT_EXPORTS_KEY = "devscope.report.exports";

const PLATFORM_LABELS: Record<PlatformKind, string> = {
  android: "Android",
  ios: "iOS",
  web: "Web",
  macos: "macOS",
  linux: "Linux",
  windows: "Windows",
};

const projectPath = ref<string | null>(null);
const logPath = ref<string | null>(null);

const report = ref<DiagnosticReport | null>(null);
const generating = ref(false);
const generateError = ref<string | null>(null);

const exporting = ref<ReportFormat | null>(null);
const exportError = ref<string | null>(null);
const exportResult = ref<string | null>(null);
const exportedPath = ref<string | null>(null);
const recentExports = ref<RecentExport[]>([]);

const comparison = ref<{ report: DiagnosticReport; path: string } | null>(null);
const comparing = ref(false);
const compareError = ref<string | null>(null);

const copying = ref<ReportFormat | null>(null);
const copyMessage = ref<string | null>(null);
const copyError = ref<string | null>(null);

const toolchainSummary = (value: DiagnosticReport): string => {
  const tools = value.toolchains ?? [];
  const available = tools
    .filter((tool) => tool.status === "ok")
    .map((tool) => `${tool.name} ${tool.version ?? ""}`.trim());

  return available.length > 0 ? available.join(" · ") : "无可用工具链";
};

const projectSummary = (value: DiagnosticReport): string => {
  if (!value.project) {
    return "未包含";
  }

  const name = value.project.name ?? "未命名";
  return value.project.version ? `${name} ${value.project.version}` : name;
};

const logSummary = (value: DiagnosticReport): string => {
  if (!value.buildLog) {
    return "未包含";
  }

  return `${value.buildLog.errorCount} 错误 / ${value.buildLog.warningCount} 警告`;
};

const diffRows = computed<DiffRow[]>(() => {
  const current = report.value;
  const previous = comparison.value?.report;
  if (!current || !previous) {
    return [];
  }

  const rows: Array<[string, string, string]> = [
    [
      "总体结论",
      reportSummaryLabel(previous.summary.status),
      reportSummaryLabel(current.summary.status),
    ],
    ["问题条目", String(previous.summary.issueCount), String(current.summary.issueCount)],
    [
      "Flutter",
      previous.flutter.version?.frameworkVersion ?? statusLabel(previous.flutter.status),
      current.flutter.version?.frameworkVersion ?? statusLabel(current.flutter.status),
    ],
    [
      "Android SDK",
      previous.android.sdkPath ?? "未找到",
      current.android.sdkPath ?? "未找到",
    ],
    [
      "Android Platforms",
      previous.android.platforms.join(", ") || "未安装",
      current.android.platforms.join(", ") || "未安装",
    ],
    ["Xcode", previous.ios.xcodeVersion ?? "未找到", current.ios.xcodeVersion ?? "未找到"],
    [
      "CocoaPods",
      previous.ios.cocoapods?.version ?? "未安装",
      current.ios.cocoapods?.version ?? "未安装",
    ],
    ["工具链", toolchainSummary(previous), toolchainSummary(current)],
    ["项目", projectSummary(previous), projectSummary(current)],
    ["日志", logSummary(previous), logSummary(current)],
  ];

  return rows.map(([label, before, after]) => ({
    label,
    before,
    after,
    changed: before !== after,
  }));
});

const changedCount = computed(() => diffRows.value.filter((row) => row.changed).length);

const systemRows = computed<DetailRow[]>(() => {
  const system = report.value?.system;
  if (!system) {
    return [];
  }

  const os = [system.osName ?? "macOS", system.osVersion]
    .filter((part): part is string => Boolean(part))
    .join(" ");

  return [
    { label: "系统", value: os || "未知" },
    { label: "Build", value: system.osBuild ?? "未知" },
    { label: "芯片", value: system.chip ?? "未知" },
    { label: "内存", value: formatMemory(system.memoryBytes) },
    { label: "CPU", value: system.cpuCores ? `${system.cpuCores} 核` : "未知" },
    { label: "机型", value: system.model ?? "未知" },
  ];
});

const flutterRows = computed<DetailRow[]>(() => {
  const flutter = report.value?.flutter;
  if (!flutter) {
    return [];
  }

  return [
    { label: "状态", value: statusLabel(flutter.status) },
    { label: "Flutter", value: flutter.version?.frameworkVersion ?? "未知" },
    { label: "Dart", value: flutter.version?.dartSdkVersion ?? "未知" },
    { label: "渠道", value: flutter.version?.channel ?? "未知" },
    {
      label: "安装路径",
      value: flutter.executablePath ?? "未找到",
      mono: true,
    },
  ];
});

const androidRows = computed<DetailRow[]>(() => {
  const android = report.value?.android;
  if (!android) {
    return [];
  }

  return [
    { label: "状态", value: statusLabel(android.status) },
    { label: "SDK", value: android.sdkPath ?? "未找到", mono: true },
    {
      label: "Platforms",
      value: android.platforms.length > 0 ? android.platforms.join(", ") : "未安装",
    },
    {
      label: "Java",
      value: android.java ? (android.java.version ?? android.java.path) : "未找到",
    },
    { label: "Licenses", value: `${android.acceptedLicenses} 个` },
    { label: "问题", value: `${android.issues.length} 项` },
  ];
});

const iosRows = computed<DetailRow[]>(() => {
  const ios = report.value?.ios;
  if (!ios) {
    return [];
  }

  return [
    { label: "状态", value: statusLabel(ios.status) },
    { label: "Xcode", value: ios.xcodeVersion ?? "未找到" },
    {
      label: "CocoaPods",
      value: ios.cocoapods?.version ?? (ios.cocoapods ? "已安装" : "未安装"),
    },
    {
      label: "模拟器",
      value: ios.simulators ? `${ios.simulators.availableCount} 台` : "未知",
    },
    { label: "问题", value: `${ios.issues.length} 项` },
  ];
});

const toolchainRows = computed<DetailRow[]>(() => {
  const tools = report.value?.toolchains ?? [];

  return tools.map((tool) => ({
    label: tool.name,
    value:
      tool.status === "ok"
        ? (tool.version ?? "版本未知")
        : tool.status === "notFound"
          ? "未安装"
          : "检测异常",
  }));
});

const projectRows = computed<DetailRow[]>(() => {
  const project = report.value?.project;
  if (!project) {
    return [];
  }

  const platforms = project.platforms
    .map((platform) => PLATFORM_LABELS[platform.platform])
    .join(", ");

  const rows: DetailRow[] = [
    { label: "状态", value: projectStatusLabel(project.status) },
    { label: "项目名", value: project.name ?? "未命名" },
    { label: "版本", value: project.version ?? "未声明" },
    { label: "平台", value: platforms || "未启用" },
    {
      label: "依赖",
      value: `${project.dependencies.length + project.devDependencies.length} 个`,
    },
    { label: "问题", value: `${project.issues.length} 项` },
  ];

  if (project.message) {
    rows.push({ label: "说明", value: project.message });
  }

  return rows;
});

const logRows = computed<DetailRow[]>(() => {
  const log = report.value?.buildLog;
  if (!log) {
    return [];
  }

  const rows: DetailRow[] = [
    { label: "状态", value: logStatusLabel(log.status) },
    { label: "文件", value: log.fileName, mono: true },
    { label: "分析行数", value: String(log.totalLines) },
    { label: "错误", value: String(log.errorCount) },
    { label: "警告", value: String(log.warningCount) },
    {
      label: "构建结果",
      value:
        log.buildOutcome === "success"
          ? "构建成功"
          : log.buildOutcome === "failure"
            ? "构建失败"
            : "未识别",
    },
  ];

  if (log.message) {
    rows.push({ label: "说明", value: log.message });
  }

  return rows;
});

const generatedAtText = computed(() => {
  const epoch = report.value?.generatedAtEpochMs;
  if (!epoch) {
    return "";
  }

  return new Date(epoch).toLocaleString("zh-CN", { hour12: false });
});

const hasInputs = computed(() => Boolean(projectPath.value || logPath.value));

function displayPath(path: string | null): string {
  return path ?? "未选择";
}

function resetReport(): void {
  report.value = null;
  generateError.value = null;
  exportError.value = null;
  exportResult.value = null;
  exportedPath.value = null;
  comparison.value = null;
  compareError.value = null;
  copyMessage.value = null;
  copyError.value = null;
}

function loadRecentExports(): void {
  try {
    const raw = localStorage.getItem(RECENT_EXPORTS_KEY);
    if (!raw) {
      return;
    }

    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed)) {
      return;
    }

    recentExports.value = parsed.filter(isRecentExport).slice(0, 5);
  } catch {
    recentExports.value = [];
  }
}

function isRecentExport(value: unknown): value is RecentExport {
  if (!value || typeof value !== "object") {
    return false;
  }

  const item = value as Record<string, unknown>;
  return (
    typeof item.path === "string" &&
    (item.format === "json" || item.format === "markdown") &&
    typeof item.at === "number"
  );
}

function rememberExport(path: string, format: ReportFormat): void {
  const entry: RecentExport = { path, format, at: Date.now() };
  const next = [
    entry,
    ...recentExports.value.filter((item) => item.path !== path),
  ].slice(0, 5);
  recentExports.value = next;

  try {
    localStorage.setItem(RECENT_EXPORTS_KEY, JSON.stringify(next));
  } catch {
    // 本地存储失败不影响导出本身。
  }
}

function formatTime(epoch: number): string {
  return new Date(epoch).toLocaleString("zh-CN", { hour12: false });
}

async function compareHistory(): Promise<void> {
  if (comparing.value) {
    return;
  }

  try {
    const path = await pickFile("选择历史诊断报告", ["json"]);
    if (!path) {
      return;
    }

    comparing.value = true;
    compareError.value = null;

    const previous = await loadDiagnosticReport(path);
    comparison.value = { report: previous, path };
  } catch (error) {
    compareError.value = toUserMessage(error, "载入历史报告失败，请选择 DevScope 导出的 JSON。");
  } finally {
    comparing.value = false;
  }
}

function clearComparison(): void {
  comparison.value = null;
  compareError.value = null;
}

async function printReport(): Promise<void> {
  if (!report.value) {
    return;
  }

  exportError.value = null;

  try {
    await printCurrentWindow();
  } catch (error) {
    exportError.value = toUserMessage(error, "无法打开打印面板。");
  }
}

async function copyReport(format: ReportFormat): Promise<void> {
  if (!report.value || copying.value) {
    return;
  }

  copying.value = format;
  copyMessage.value = null;
  copyError.value = null;

  try {
    const text = await renderDiagnosticReport(report.value, format);
    await copyText(text);
    copyMessage.value =
      format === "markdown" ? "Markdown 已复制到剪贴板" : "JSON 已复制到剪贴板";
  } catch (error) {
    copyError.value = toUserMessage(error, "复制报告失败。");
  } finally {
    copying.value = null;
  }
}

async function pickProject(): Promise<void> {
  try {
    const path = await pickDirectory("选择要包含在报告中的 Flutter 项目");
    if (!path) {
      return;
    }

    projectPath.value = path;
    resetReport();
  } catch (error) {
    generateError.value = toUserMessage(error, "无法打开目录选择器。");
  }
}

async function pickLog(): Promise<void> {
  try {
    const path = await pickFile("选择要包含在报告中的构建日志", []);
    if (!path) {
      return;
    }

    logPath.value = path;
    resetReport();
  } catch (error) {
    generateError.value = toUserMessage(error, "无法打开文件选择器。");
  }
}

function clearProject(): void {
  projectPath.value = null;
  resetReport();
}

function clearLog(): void {
  logPath.value = null;
  resetReport();
}

async function generate(): Promise<void> {
  if (generating.value) {
    return;
  }

  generating.value = true;
  generateError.value = null;
  exportError.value = null;
  exportResult.value = null;
  exportedPath.value = null;
  comparison.value = null;
  compareError.value = null;
  copyMessage.value = null;
  copyError.value = null;

  const force = report.value !== null;

  try {
    report.value = await generateDiagnosticReport(
      {
        projectPath: projectPath.value,
        logPath: logPath.value,
      },
      force,
    );
  } catch (error) {
    report.value = null;
    generateError.value = toUserMessage(error, "生成诊断报告失败，请查看应用日志。");
  } finally {
    generating.value = false;
  }
}

function defaultFileName(format: ReportFormat): string {
  const epoch = report.value?.generatedAtEpochMs ?? Date.now();
  const stamp = new Date(epoch).toISOString().slice(0, 19).replace(/[:T]/g, "-");

  return `devscope-report-${stamp}.${format === "json" ? "json" : "md"}`;
}

async function exportReport(format: ReportFormat): Promise<void> {
  if (!report.value || exporting.value) {
    return;
  }

  exporting.value = format;
  exportError.value = null;
  exportResult.value = null;
  exportedPath.value = null;
  copyMessage.value = null;
  copyError.value = null;

  try {
    const path = await pickSavePath("导出诊断报告", defaultFileName(format), format);
    if (!path) {
      return;
    }

    const saved = await saveDiagnosticReport(report.value, format, path);
    exportResult.value = `已导出 ${saved.bytes} 字节到 ${saved.path}`;
    exportedPath.value = saved.path;
    rememberExport(saved.path, saved.format);
  } catch (error) {
    exportError.value = toUserMessage(error, "导出诊断报告失败，请查看应用日志。");
  } finally {
    exporting.value = null;
  }
}
onMounted(() => {
  loadRecentExports();
});
</script>

<template>
  <section class="page">
    <header class="page-header">
      <div class="page-heading">
        <p class="eyebrow">诊断报告</p>
        <h1>导出诊断报告</h1>
        <p class="subtitle">
          汇总本机配置、Flutter / Android / iOS 环境，可选加入项目分析与构建日志，
          导出为 JSON 或 Markdown。所有检测都在本地完成。
        </p>
      </div>

      <button
        class="refresh refresh--primary"
        type="button"
        :disabled="generating"
        @click="generate"
      >
        <span v-if="generating" class="spinner spinner--small" aria-hidden="true"></span>
        {{ generating ? "生成中…" : report ? "重新生成" : "生成报告" }}
      </button>
    </header>

    <div class="card options-card">
      <div class="card-head">
        <div>
          <h2>报告内容</h2>
          <p class="card-subtitle">
            项目与日志是可选项；不选择时报告只包含本机环境检测。
          </p>
        </div>
      </div>

      <div class="option-list">
        <div class="option-row">
          <div class="option-copy">
            <span class="option-label">Flutter 项目</span>
            <code class="option-path" :class="{ 'option-path--empty': !projectPath }">
              {{ displayPath(projectPath) }}
            </code>
          </div>
          <div class="option-actions">
            <button class="refresh" type="button" :disabled="generating" @click="pickProject">
              选择
            </button>
            <button
              v-if="projectPath"
              class="refresh"
              type="button"
              :disabled="generating"
              @click="clearProject"
            >
              清除
            </button>
          </div>
        </div>

        <div class="option-row">
          <div class="option-copy">
            <span class="option-label">构建日志</span>
            <code class="option-path" :class="{ 'option-path--empty': !logPath }">
              {{ displayPath(logPath) }}
            </code>
          </div>
          <div class="option-actions">
            <button class="refresh" type="button" :disabled="generating" @click="pickLog">
              选择
            </button>
            <button
              v-if="logPath"
              class="refresh"
              type="button"
              :disabled="generating"
              @click="clearLog"
            >
              清除
            </button>
          </div>
        </div>
      </div>

      <p class="note">
        生成报告会执行 Flutter / Android / iOS 三套检测，可能需要数秒到数十秒。
      </p>
    </div>

    <div v-if="generateError" class="callout callout--error" role="alert">
      {{ generateError }}
    </div>

    <div v-if="generating" class="card state-card" role="status" aria-live="polite">
      <span class="spinner" aria-hidden="true"></span>
      <div>
        <h2>正在生成诊断报告…</h2>
        <p>正在执行环境检测{{ hasInputs ? "并读取项目 / 日志" : "" }}，请稍候。</p>
      </div>
    </div>

    <template v-if="report && !generating">
      <div class="card">
        <div class="card-head">
          <div>
            <h2>诊断摘要</h2>
            <p class="card-subtitle">
              生成于 {{ generatedAtText }} · {{ report.generator }}
            </p>
          </div>
          <div class="summary-actions">
            <button
              class="refresh"
              type="button"
              :disabled="comparing"
              @click="compareHistory"
            >
              <span v-if="comparing" class="spinner spinner--small" aria-hidden="true"></span>
              {{ comparing ? "载入中…" : comparison ? "更换对比" : "对比历史报告" }}
            </button>
            <StatusBadge
              :tone="statusTone(report.summary.status)"
              :label="reportSummaryLabel(report.summary.status)"
            />
          </div>
        </div>

        <ul class="summary-list">
          <li v-for="highlight in report.summary.highlights" :key="highlight">
            {{ highlight }}
          </li>
        </ul>

        <div class="metrics">
          <div class="metric">
            <span class="metric-label">问题条目</span>
            <span class="metric-value">{{ report.summary.issueCount }}</span>
          </div>
          <div class="metric">
            <span class="metric-label">结构版本</span>
            <span class="metric-value">{{ report.schemaVersion }}</span>
          </div>
          <div class="metric">
            <span class="metric-label">可选内容</span>
            <span class="metric-value">
              {{ [report.project ? "项目" : null, report.buildLog ? "日志" : null].filter(Boolean).join(" + ") || "无" }}
            </span>
          </div>
        </div>
      </div>

      <div v-if="compareError" class="callout callout--error" role="alert">
        {{ compareError }}
      </div>

      <div v-if="comparison" class="card">
        <div class="card-head">
          <div>
            <h2>与历史报告对比</h2>
            <p class="card-subtitle">
              {{ formatTime(comparison.report.generatedAtEpochMs) }} ·
              <code>{{ comparison.path }}</code>
            </p>
          </div>
          <div class="summary-actions">
            <StatusBadge
              :tone="changedCount > 0 ? 'warning' : 'ok'"
              :label="changedCount > 0 ? `${changedCount} 项变化` : '没有变化'"
            />
            <button class="refresh" type="button" @click="clearComparison">清除对比</button>
          </div>
        </div>

        <div v-if="changedCount === 0" class="empty-note">
          两份报告在摘要、环境版本、工具链、项目和日志统计上没有差异。
        </div>
        <div v-else class="table-wrap">
          <table class="diff-table">
            <thead>
              <tr>
                <th>项目</th>
                <th>历史</th>
                <th>当前</th>
                <th></th>
              </tr>
            </thead>
            <tbody>
              <tr
                v-for="row in diffRows"
                :key="row.label"
                :class="{ 'diff-row--changed': row.changed }"
              >
                <td class="diff-label">{{ row.label }}</td>
                <td>{{ row.before }}</td>
                <td>{{ row.after }}</td>
                <td class="diff-flag">{{ row.changed ? "变化" : "—" }}</td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>

      <div class="grid-2">
        <div class="card">
          <h2 class="section-title">本机配置</h2>
          <DetailList :rows="systemRows" />
        </div>
        <div class="card">
          <h2 class="section-title">Flutter 环境</h2>
          <DetailList :rows="flutterRows" />
        </div>
        <div class="card">
          <h2 class="section-title">Android 环境</h2>
          <DetailList :rows="androidRows" />
        </div>
        <div class="card">
          <h2 class="section-title">iOS / Xcode 环境</h2>
          <DetailList :rows="iosRows" />
        </div>
        <div v-if="report.toolchains.length > 0" class="card">
          <h2 class="section-title">常用工具链</h2>
          <DetailList :rows="toolchainRows" />
        </div>
        <div v-if="report.project" class="card">
          <h2 class="section-title">Flutter 项目</h2>
          <DetailList :rows="projectRows" />
        </div>
        <div v-if="report.buildLog" class="card">
          <h2 class="section-title">构建日志</h2>
          <DetailList :rows="logRows" />
        </div>
      </div>

      <div class="card export-card">
        <div class="card-head">
          <div>
            <h2>导出</h2>
            <p class="card-subtitle">
              JSON 适合程序处理，Markdown 适合直接阅读或贴进 issue。
            </p>
          </div>
        </div>

        <div class="export-actions">
          <button
            class="refresh"
            type="button"
            :disabled="exporting !== null"
            @click="exportReport('json')"
          >
            <span
              v-if="exporting === 'json'"
              class="spinner spinner--small"
              aria-hidden="true"
            ></span>
            {{ exporting === "json" ? "导出中…" : "导出 JSON" }}
          </button>
          <button
            class="refresh refresh--primary"
            type="button"
            :disabled="exporting !== null"
            @click="exportReport('markdown')"
          >
            <span
              v-if="exporting === 'markdown'"
              class="spinner spinner--small"
              aria-hidden="true"
            ></span>
            {{ exporting === "markdown" ? "导出中…" : "导出 Markdown" }}
          </button>
          <button
            class="refresh"
            type="button"
            :disabled="copying !== null"
            @click="copyReport('markdown')"
          >
            <span
              v-if="copying === 'markdown'"
              class="spinner spinner--small"
              aria-hidden="true"
            ></span>
            {{ copying === "markdown" ? "复制中…" : "复制 Markdown" }}
          </button>
          <button
            class="refresh"
            type="button"
            :disabled="copying !== null"
            @click="copyReport('json')"
          >
            <span
              v-if="copying === 'json'"
              class="spinner spinner--small"
              aria-hidden="true"
            ></span>
            {{ copying === "json" ? "复制中…" : "复制 JSON" }}
          </button>
          <button class="refresh" type="button" @click="printReport">
            打印 / 存为 PDF
          </button>
        </div>

        <div v-if="exportError" class="callout callout--error" role="alert">
          {{ exportError }}
        </div>
        <div v-else-if="exportResult" class="callout callout--success" role="status">
          {{ exportResult }}
        </div>

        <div v-if="exportedPath" class="export-reveal">
          <PathActions :path="exportedPath" />
        </div>

        <div v-if="copyError" class="callout callout--error" role="alert">
          {{ copyError }}
        </div>
        <div v-else-if="copyMessage" class="callout callout--success" role="status">
          {{ copyMessage }}
        </div>

        <div v-if="recentExports.length > 0" class="recent-exports">
          <h3>最近导出</h3>
          <ul>
            <li v-for="item in recentExports" :key="item.path" class="recent-item">
              <div class="recent-meta">
                <span>{{ item.format === "json" ? "JSON" : "Markdown" }}</span>
                <span>{{ formatTime(item.at) }}</span>
              </div>
              <code>{{ item.path }}</code>
              <PathActions :path="item.path" />
            </li>
          </ul>
        </div>
      </div>
    </template>
  </section>
</template>

<style scoped>
.option-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
  margin-top: 16px;
}

.option-row {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 12px 14px;
  border: 1px solid var(--border);
  border-radius: 10px;
  background: var(--surface-soft);
}

.option-copy {
  display: flex;
  min-width: 0;
  flex: 1;
  flex-direction: column;
  gap: 4px;
}

.option-label {
  color: var(--text-secondary);
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.06em;
}

.option-path {
  padding: 0;
  overflow-wrap: anywhere;
  background: transparent;
  color: var(--text);
  font-size: 11.5px;
}

.option-path--empty {
  color: var(--text-tertiary);
}

.option-actions {
  display: flex;
  flex-shrink: 0;
  gap: 6px;
}

.note {
  margin: 14px 0 0;
  color: var(--text-tertiary);
  font-size: 11.5px;
  line-height: 1.6;
}

.summary-list {
  margin: 14px 0 0;
  padding: 0;
  list-style: none;
}

.summary-list li {
  position: relative;
  padding: 6px 0 6px 14px;
  color: var(--text-secondary);
  font-size: 12px;
  line-height: 1.55;
  overflow-wrap: anywhere;
}

.summary-list li::before {
  position: absolute;
  top: 12px;
  left: 1px;
  width: 4px;
  height: 4px;
  border-radius: 50%;
  background: var(--text-tertiary);
  content: "";
}

.export-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: 14px;
}

.export-reveal {
  margin-top: 10px;
}

.summary-actions {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px;
}

.diff-table {
  width: 100%;
  margin-top: 14px;
  border-collapse: collapse;
  font-size: 11.5px;
}

.diff-table th {
  padding: 0 6px 7px;
  border-bottom: 1px solid var(--border);
  color: var(--text-tertiary);
  font-size: 9.5px;
  font-weight: 700;
  letter-spacing: 0.06em;
  text-align: left;
  text-transform: uppercase;
}

.diff-table td {
  padding: 8px 6px;
  border-bottom: 1px solid var(--border-soft);
  color: var(--text-secondary);
  overflow-wrap: anywhere;
}

.diff-table tr:last-child td {
  border-bottom: 0;
}

.diff-row--changed td {
  background: var(--warning-soft);
}

.diff-row--changed .diff-label,
.diff-row--changed .diff-flag {
  color: var(--warning);
  font-weight: 650;
}

.diff-flag {
  white-space: nowrap;
}

.recent-exports {
  margin-top: 18px;
  padding-top: 14px;
  border-top: 1px solid var(--border);
}

.recent-exports h3 {
  margin: 0;
  color: var(--text-secondary);
  font-size: 12px;
  font-weight: 650;
}

.recent-exports ul {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin: 10px 0 0;
  padding: 0;
  list-style: none;
}

.recent-item {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.recent-meta {
  display: flex;
  gap: 8px;
  color: var(--text-tertiary);
  font-size: 10px;
}

.recent-item code {
  overflow-wrap: anywhere;
}

.callout--success {
  border-color: var(--ok-border);
  background: var(--ok-soft);
  color: var(--ok);
}

@media (max-width: 680px) {
  .option-row {
    align-items: flex-start;
    flex-direction: column;
  }
}
</style>
