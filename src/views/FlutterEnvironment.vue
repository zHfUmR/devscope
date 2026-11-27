<script setup lang="ts">
/**
 * Flutter 环境检测页。
 *
 * 页面只负责组合状态和展示；所有系统探测都通过 services/rust-api.ts 的 IPC
 * 封装交给 Rust。四种状态：
 *   loading  等待 Rust 返回
 *   ok       找到可用的 Flutter SDK
 *   notFound 没有找到 Flutter（空状态）
 *   error    找到了但执行/解析失败，或 IPC 本身失败
 */
import { computed, onMounted } from "vue";
import { RouterLink } from "vue-router";
import { useDetection } from "../composables/useDetection";
import StatusBadge from "../components/StatusBadge.vue";
import { detectFlutterEnvironment, runFlutterDoctor } from "../services/rust-api";
import type { StatusTone } from "../services/status";
import type { DoctorSection, DoctorStatus, FlutterExecutableSource } from "../types";
import type { DetailRow, Metric } from "../types/ui";

type ViewState = "loading" | "ipcError" | "ok" | "notFound" | "error";

interface DoctorBadge {
  tone: StatusTone;
  label: string;
}

/** doctor 检查项与对应详情页的链接。 */
interface DoctorRow {
  section: DoctorSection;
  link: { to: string; label: string } | null;
}

const SOURCE_LABELS: Record<FlutterExecutableSource, string> = {
  path: "PATH",
  commonLocation: "常见安装目录",
  loginShell: "登录 Shell",
};

const {
  data: environment,
  loading,
  started,
  ipcError,
  refresh,
} = useDetection(detectFlutterEnvironment);

/**
 * 按需运行 flutter doctor。
 *
 * doctor 会检查设备、网络和多个工具链，耗时明显长于版本检测，所以不自动执行，
 * 也不复用主检测的 loading 状态。
 */
const {
  data: doctorReport,
  loading: doctorLoading,
  ipcError: doctorError,
  refresh: runDoctor,
} = useDetection(runFlutterDoctor, "运行 flutter doctor 失败，请查看应用日志。");

/** 第一次加载完成前也显示 loading，避免页面先闪一下空内容。 */
const viewState = computed<ViewState>(() => {
  if (!started.value || loading.value) {
    return "loading";
  }
  if (ipcError.value) {
    return "ipcError";
  }

  return environment.value?.status ?? "error";
});

/** 成功状态下的三项核心信息。 */
const metrics = computed<Metric[]>(() => {
  const version = environment.value?.version;
  if (!version) {
    return [];
  }

  return [
    { label: "Flutter", value: version.frameworkVersion },
    { label: "Dart", value: version.dartSdkVersion ?? "未提供" },
    { label: "发布渠道", value: version.channel ?? "未提供" },
  ];
});

/** 安装详情；只展示有值的字段，避免一堆「未知」。 */
const details = computed<DetailRow[]>(() => {
  const result = environment.value;
  const version = result?.version;
  const rows: DetailRow[] = [];

  if (result?.executablePath) {
    rows.push({ label: "安装路径", value: result.executablePath, mono: true });
  }
  if (result?.executableSource) {
    rows.push({ label: "发现方式", value: SOURCE_LABELS[result.executableSource] });
  }
  if (version?.devToolsVersion) {
    rows.push({ label: "DevTools", value: version.devToolsVersion });
  }
  if (version?.engineRevision) {
    rows.push({ label: "Engine Revision", value: version.engineRevision, mono: true });
  }
  if (version?.frameworkRevision) {
    rows.push({ label: "Framework Revision", value: version.frameworkRevision, mono: true });
  }
  if (version?.frameworkCommitDate) {
    rows.push({ label: "Framework 提交时间", value: version.frameworkCommitDate });
  }

  return rows;
});

const checkedPaths = computed<string[]>(() => environment.value?.checkedPaths ?? []);

const failureMessage = computed(
  () => environment.value?.message ?? "检测失败，但没有返回具体原因。",
);

const executablePath = computed(() => environment.value?.executablePath ?? null);

/** doctor 的 section 列表；用 computed 避免模板里反复处理 null。 */
const doctorSections = computed(() => doctorReport.value?.sections ?? []);
const doctorRawOutput = computed(() => doctorReport.value?.rawOutput ?? "");
const doctorMessage = computed(() => doctorReport.value?.message ?? null);
const doctorTruncated = computed(() => doctorReport.value?.truncated ?? false);

/**
 * 把 doctor 的 Android / Xcode 检查项关联到同级详情页。
 *
 * doctor 是 Flutter 视角的摘要，详情页才是权威；这里只做跳转，不重复检测。
 */
function doctorLink(title: string): DoctorRow["link"] {
  const normalized = title.toLowerCase();

  if (normalized.startsWith("android")) {
    return { to: "/environment/android", label: "查看 Android 详情" };
  }
  if (normalized.startsWith("xcode")) {
    return { to: "/environment/ios", label: "查看 iOS 详情" };
  }

  return null;
}

const doctorRows = computed<DoctorRow[]>(() =>
  doctorSections.value.map((section) => ({
    section,
    link: doctorLink(section.title),
  })),
);

/** 整体体检徽标：优先展示失败/注意数量，比只用 status 更有信息量。 */
const doctorSummary = computed<DoctorBadge>(() => {
  const report = doctorReport.value;
  if (!report) {
    return { tone: "warning", label: "尚未体检" };
  }

  const errors = report.sections.filter((section) => section.status === "error").length;
  const warnings = report.sections.filter((section) => section.status === "warning").length;

  if (errors > 0) {
    return { tone: "error", label: `${errors} 项失败` };
  }
  if (warnings > 0) {
    return { tone: "warning", label: `${warnings} 项注意` };
  }
  if (report.status === "error") {
    return { tone: "error", label: "体检失败" };
  }
  if (report.status === "warning") {
    return { tone: "warning", label: "有注意事项" };
  }

  return { tone: "ok", label: "全部通过" };
});

/** 单个 section 的中文状态文案。 */
function sectionLabel(status: DoctorStatus): string {
  if (status === "ok") {
    return "通过";
  }
  if (status === "warning") {
    return "注意";
  }

  return "失败";
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
        <h1>Flutter</h1>
        <p class="subtitle">
          检查本机 Flutter SDK 是否可用，并读取版本、发布渠道和安装位置。
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
        <h2>正在检测 Flutter 环境…</h2>
        <p>正在定位 flutter 并读取版本信息，请稍候。</p>
      </div>
    </div>

    <div
      v-else-if="viewState === 'ipcError'"
      class="card state-card state-card--error"
      role="alert"
    >
      <div class="state-icon state-icon--error" aria-hidden="true">!</div>
      <div class="state-body">
        <h2>无法完成检测</h2>
        <p>{{ ipcError }}</p>
        <p class="hint">
          如果你是在浏览器中直接打开页面，请改用 <code>npm run tauri dev</code> 启动桌面应用。
        </p>
      </div>
    </div>

    <template v-else-if="viewState === 'ok'">
      <div class="card">
        <div class="card-head">
          <div>
            <h2>Flutter SDK</h2>
            <p class="card-subtitle">已找到可用的 Flutter 安装。</p>
          </div>
          <StatusBadge tone="ok" label="已就绪" />
        </div>

        <div class="metrics">
          <div v-for="metric in metrics" :key="metric.label" class="metric">
            <span class="metric-label">{{ metric.label }}</span>
            <span class="metric-value">{{ metric.value }}</span>
          </div>
        </div>
      </div>

      <div class="card">
        <h2 class="section-title">安装详情</h2>
        <dl class="details">
          <div v-for="row in details" :key="row.label" class="detail-row">
            <dt>{{ row.label }}</dt>
            <dd :class="{ mono: row.mono }">{{ row.value }}</dd>
          </div>
        </dl>
      </div>

      <div class="card doctor-card">
        <div class="card-head">
          <div>
            <h2>Flutter 工具链体检</h2>
            <p class="card-subtitle">
              运行 flutter doctor -v，检查 Android、iOS、设备和网络。通常需要数秒到数十秒。
            </p>
          </div>
          <button
            class="refresh doctor-run"
            type="button"
            :disabled="doctorLoading"
            @click="runDoctor()"
          >
            <span v-if="doctorLoading" class="spinner spinner--small" aria-hidden="true"></span>
            {{ doctorLoading ? "体检中…" : doctorReport ? "重新体检" : "运行体检" }}
          </button>
        </div>

        <div v-if="doctorLoading" class="doctor-loading" role="status" aria-live="polite">
          <span class="spinner spinner--small" aria-hidden="true"></span>
          正在运行 flutter doctor -v，请稍候…
        </div>

        <div
          v-else-if="doctorError"
          class="doctor-message doctor-message--error"
          role="alert"
        >
          {{ doctorError }}
        </div>

        <div v-else-if="!doctorReport" class="doctor-empty">
          doctor 会检查 Flutter、Android、Xcode、Chrome、设备和网络。首次运行可能较慢。
        </div>

        <template v-else>
          <div class="doctor-summary">
            <StatusBadge :tone="doctorSummary.tone" :label="doctorSummary.label" />
            <span v-if="doctorSections.length > 0" class="doctor-count">
              {{ doctorSections.length }} 项检查
            </span>
            <span v-if="doctorTruncated" class="doctor-note">输出已截断</span>
          </div>

          <p
            v-if="doctorMessage"
            class="doctor-message"
            :class="{ 'doctor-message--error': doctorSummary.tone === 'error' }"
          >
            {{ doctorMessage }}
          </p>

          <ul v-if="doctorRows.length > 0" class="doctor-sections">
            <li
              v-for="(row, index) in doctorRows"
              :key="`${row.section.title}-${index}`"
              class="doctor-section"
            >
              <StatusBadge
                :tone="row.section.status"
                :label="sectionLabel(row.section.status)"
              />
              <div class="doctor-section-body">
                <span class="doctor-section-title">{{ row.section.title }}</span>
                <span v-if="row.section.summary" class="doctor-section-summary">
                  {{ row.section.summary }}
                </span>
              </div>
              <RouterLink v-if="row.link" class="doctor-link" :to="row.link.to">
                {{ row.link.label }}
              </RouterLink>
            </li>
          </ul>

          <details v-if="doctorRawOutput" class="doctor-raw">
            <summary>查看 flutter doctor -v 输出</summary>
            <pre>{{ doctorRawOutput }}</pre>
          </details>
        </template>
      </div>
    </template>

    <div v-else-if="viewState === 'notFound'" class="card state-card">
      <div class="state-icon state-icon--empty" aria-hidden="true">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8">
          <circle cx="10.5" cy="10.5" r="6.5" />
          <path d="m15.4 15.4 4.1 4.1" stroke-linecap="round" />
        </svg>
      </div>

      <div class="state-body">
        <div class="state-heading">
          <h2>未检测到 Flutter SDK</h2>
          <StatusBadge tone="warning" label="未安装" />
        </div>
        <p>{{ failureMessage }}</p>
        <p class="hint">
          安装后请确认 <code>flutter</code> 在 PATH 中，或放在常见目录（例如
          <code>~/development/flutter</code>）。
        </p>

        <details v-if="checkedPaths.length > 0" class="checked">
          <summary>查看已检查的 {{ checkedPaths.length }} 个位置</summary>
          <ul>
            <li v-for="path in checkedPaths" :key="path">
              <code>{{ path }}</code>
            </li>
          </ul>
        </details>
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
          <h2>Flutter 检测失败</h2>
          <StatusBadge tone="error" label="检测异常" />
        </div>
        <p>{{ failureMessage }}</p>

        <p v-if="executablePath" class="hint">
          检测到的路径：<code>{{ executablePath }}</code>
        </p>

        <details v-if="checkedPaths.length > 0" class="checked">
          <summary>查看已检查的 {{ checkedPaths.length }} 个位置</summary>
          <ul>
            <li v-for="path in checkedPaths" :key="path">
              <code>{{ path }}</code>
            </li>
          </ul>
        </details>
      </div>
    </div>
  </section>
</template>

<style scoped>
.doctor-loading {
  display: flex;
  align-items: center;
  gap: 9px;
  margin-top: 18px;
  color: var(--text-secondary);
  font-size: 12px;
}

.doctor-empty {
  margin-top: 16px;
  padding: 16px;
  border: 1px dashed var(--border);
  border-radius: 10px;
  background: var(--surface-soft);
  color: var(--text-secondary);
  font-size: 12px;
  line-height: 1.6;
}

.doctor-summary {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 10px;
  margin-top: 18px;
}

.doctor-count {
  color: var(--text-tertiary);
  font-size: 12px;
}

.doctor-note {
  color: var(--warning);
  font-size: 12px;
}

.doctor-message {
  margin: 12px 0 0;
  padding: 11px 13px;
  border-radius: 9px;
  background: var(--surface-soft);
  color: var(--text-secondary);
  font-size: 12px;
  line-height: 1.6;
  overflow-wrap: anywhere;
}

.doctor-message--error {
  background: var(--error-soft);
  color: var(--error);
}

.doctor-sections {
  margin: 14px 0 0;
  padding: 0;
  list-style: none;
}

.doctor-section {
  display: flex;
  align-items: flex-start;
  gap: 12px;
  padding: 11px 0;
  border-top: 1px solid var(--border);
}

.doctor-section:first-child {
  border-top: 0;
}

.doctor-section-body {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 3px;
  min-width: 0;
}

.doctor-section-title {
  color: var(--text);
  font-size: 13px;
  font-weight: 600;
  line-height: 1.45;
  overflow-wrap: anywhere;
}

.doctor-section-summary {
  color: var(--text-secondary);
  font-size: 12px;
  line-height: 1.55;
  overflow-wrap: anywhere;
}

.doctor-link {
  flex-shrink: 0;
  padding-top: 2px;
  font-size: 11.5px;
  font-weight: 600;
  white-space: nowrap;
}

.doctor-raw {
  margin-top: 16px;
  padding-top: 12px;
  border-top: 1px solid var(--border);
}

.doctor-raw summary {
  color: var(--text-secondary);
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
}

.doctor-raw pre {
  max-height: 420px;
  margin: 11px 0 0;
  padding: 14px;
  overflow: auto;
  border: 1px solid var(--border);
  border-radius: 10px;
  background: var(--surface-code-block);
  color: var(--code-text);
  font-family: "SFMono-Regular", ui-monospace, "SF Mono", Menlo, monospace;
  font-size: 11.5px;
  line-height: 1.55;
  white-space: pre;
}

@media (max-width: 680px) {
  .doctor-section {
    flex-wrap: wrap;
  }

  .doctor-link {
    margin-left: 32px;
  }
}
</style>
