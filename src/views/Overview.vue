<script setup lang="ts">
/**
 * 总览页：本机配置 + 常用开发环境 + 常用语言。
 *
 * 不展示各模块的完整检测细节；点击环境行或语言卡片会跳到左侧对应的
 * 环境检测子项。四个检测并行执行，任何一个失败只影响它自己的展示。
 */
import { computed, onMounted, ref, type Ref } from "vue";
import { RouterLink } from "vue-router";
import EnvironmentRow from "../components/EnvironmentRow.vue";
import StatusBadge from "../components/StatusBadge.vue";
import { toUserMessage } from "../services/errors";
import { formatMemory } from "../services/format";
import { statusLabel, statusTone, type StatusTone } from "../services/status";
import {
  detectAndroidEnvironment,
  detectFlutterEnvironment,
  detectIosEnvironment,
  detectSystemInfo,
  detectToolchains,
} from "../services/rust-api";
import type {
  AndroidEnvironment,
  FlutterEnvironment,
  IosEnvironment,
  SystemInfo,
  ToolchainInfo,
  ToolchainKind,
} from "../types";
import type { Metric } from "../types/ui";


interface Loadable<T> {
  loading: boolean;
  data: T | null;
  error: string | null;
}

interface Language {
  name: string;
  detail: string;
  version: string;
  env: string;
  to: string;
}

const system = ref<Loadable<SystemInfo>>(loadingState());
const flutter = ref<Loadable<FlutterEnvironment>>(loadingState());
const android = ref<Loadable<AndroidEnvironment>>(loadingState());
const ios = ref<Loadable<IosEnvironment>>(loadingState());
const toolchains = ref<Loadable<ToolchainInfo[]>>(loadingState());
const started = ref(false);

function loadingState<T>(): Loadable<T> {
  return { loading: true, data: null, error: null };
}

const envLoading = computed(
  () => flutter.value.loading || android.value.loading || ios.value.loading,
);

/** 首次挂载时 loading 初始为 true，因此不能直接用 loading 当「正在刷新」守卫。 */
const loading = computed(
  () => system.value.loading || envLoading.value || toolchains.value.loading,
);

/** 单个模块完成后立即更新自己的卡片，不必等其余模块一起结束。 */
function loadInto<T>(target: Ref<Loadable<T>>, promise: Promise<T>): Promise<void> {
  return promise
    .then((data) => {
      target.value = { loading: false, data, error: null };
    })
    .catch((error: unknown) => {
      target.value = {
        loading: false,
        data: null,
        error: toUserMessage(error, "调用 Rust 命令失败，请查看应用日志。"),
      };
    });
}

/** 四个检测并行执行；某一个失败只影响它自己的展示。 */
async function refresh(force = false): Promise<void> {
  if (started.value && loading.value) {
    return;
  }
  started.value = true;

  system.value = loadingState();
  flutter.value = loadingState();
  android.value = loadingState();
  ios.value = loadingState();
  toolchains.value = loadingState();

  await Promise.allSettled([
    loadInto(system, detectSystemInfo(force)),
    loadInto(flutter, detectFlutterEnvironment(force)),
    loadInto(android, detectAndroidEnvironment(force)),
    loadInto(ios, detectIosEnvironment(force)),
    loadInto(toolchains, detectToolchains(force)),
  ]);
}

onMounted(() => {
  void refresh();
});

const systemSubtitle = computed(() => {
  if (system.value.loading) {
    return "正在读取本机配置…";
  }
  if (system.value.error) {
    return "本机配置读取失败";
  }

  const data = system.value.data;
  const host = data?.hostname ?? "本机";
  return data?.osBuild ? `${host} · Build ${data.osBuild}` : host;
});

const systemMetrics = computed<Metric[]>(() => {
  const data = system.value.data;

  return [
    {
      label: "系统",
      value: data?.osVersion ? `${data.osName ?? "macOS"} ${data.osVersion}` : "未知",
    },
    { label: "芯片", value: data?.chip ?? "未知" },
    { label: "内存", value: formatMemory(data?.memoryBytes ?? null) },
    { label: "CPU", value: data?.cpuCores ? `${data.cpuCores} 核` : "未知" },
    { label: "架构", value: data?.architecture ?? "未知" },
    { label: "机型", value: data?.model ?? "未知" },
  ];
});

/** 开发环境区域的整体状态，只做摘要，不重复详情。 */
const envStatus = computed<{ tone: StatusTone; label: string }>(() => {
  if (envLoading.value) {
    return { tone: "warning", label: "检测中" };
  }

  const states = [flutter.value, android.value, ios.value];
  if (states.some((state) => state.error !== null)) {
    return { tone: "error", label: "需处理" };
  }

  const statuses = states.map((state) => state.data?.status);
  if (statuses.some((status) => status === "error")) {
    return { tone: "error", label: "需处理" };
  }
  if (flutter.value.data?.status === "notFound") {
    return { tone: "warning", label: "未就绪" };
  }
  if (statuses.some((status) => status === "warning" || status === "notFound")) {
    return { tone: "warning", label: "基本可用" };
  }

  return { tone: "ok", label: "已就绪" };
});

function flutterHeadline(): string {
  const data = flutter.value.data;
  if (!data) {
    return "";
  }
  if (data.status === "notFound") {
    return "未检测到 Flutter SDK";
  }
  if (data.status === "error") {
    return data.message ?? "Flutter 检测失败";
  }

  const version = data.version?.frameworkVersion ?? "未知版本";
  const channel = data.version?.channel ?? "未知渠道";
  return `Flutter ${version} · ${channel}`;
}

function androidHeadline(): string {
  const data = android.value.data;
  if (!data) {
    return "";
  }
  if (data.status === "notFound") {
    return "未检测到 Android SDK";
  }
  if (data.status === "error") {
    return data.message ?? "Android 检测失败";
  }

  const java = data.java?.version ? `Java ${data.java.version}` : "未找到 JDK";
  return `${data.platforms[0] ?? "无 platform"} · ${java}`;
}

function iosHeadline(): string {
  const data = ios.value.data;
  if (!data) {
    return "";
  }
  if (data.status === "notFound") {
    return "未检测到 Xcode";
  }
  if (data.status === "error") {
    return data.message ?? "iOS 检测失败";
  }

  const version = data.xcodeVersion ? `Xcode ${data.xcodeVersion}` : "Xcode";
  const simulators = data.simulators
    ? `${data.simulators.availableCount} 台模拟器`
    : "模拟器未知";
  return `${version} · ${simulators}`;
}

function issuesMeta(issues: string[] | undefined): string {
  const count = issues?.length ?? 0;
  return count > 0 ? `${count} 项需注意` : "";
}

const toolchainList = computed<ToolchainInfo[]>(() => toolchains.value.data ?? []);

const availableToolchains = computed(
  () => toolchainList.value.filter((tool) => tool.status === "ok").length,
);

const toolchainBadge = computed<{ tone: StatusTone; label: string }>(() => {
  if (toolchains.value.loading) {
    return { tone: "warning", label: "检测中" };
  }
  if (toolchains.value.error) {
    return { tone: "error", label: "调用失败" };
  }

  const total = toolchainList.value.length;
  if (total === 0) {
    return { tone: "warning", label: "无结果" };
  }
  if (toolchainList.value.some((tool) => tool.status === "error")) {
    return { tone: "error", label: "需处理" };
  }
  if (availableToolchains.value === 0) {
    return { tone: "warning", label: "未安装" };
  }
  if (availableToolchains.value < total) {
    return { tone: "warning", label: `${availableToolchains.value}/${total} 可用` };
  }

  return { tone: "ok", label: "全部可用" };
});

const toolchainHeadline = computed(() => {
  if (toolchains.value.loading || toolchains.value.error) {
    return "";
  }

  const installed = toolchainList.value.filter((tool) => tool.status === "ok");
  if (installed.length === 0) {
    return "未检测到常用工具链";
  }

  const preview = installed
    .slice(0, 3)
    .map((tool) => `${tool.name} ${tool.version ?? ""}`.trim())
    .join(" · ");

  return installed.length > 3 ? `${preview} 等 ${installed.length} 个` : preview;
});

function toolchainVersion(kind: ToolchainKind): string {
  if (toolchains.value.loading) {
    return "检测中…";
  }
  if (toolchains.value.error) {
    return "检测失败";
  }

  const tool = toolchainList.value.find((item) => item.kind === kind);
  if (!tool) {
    return "未检测";
  }
  if (tool.status === "notFound") {
    return "未安装";
  }
  if (tool.status === "error") {
    return "检测异常";
  }

  return tool.version ?? "版本未知";
}

function languageVersion(
  loading: boolean,
  error: string | null,
  version: string | null | undefined,
): string {
  if (loading) {
    return "检测中…";
  }
  if (error) {
    return "检测失败";
  }

  return version ?? "未检测";
}

const languages = computed<Language[]>(() => [
  {
    name: "Dart",
    detail: "Flutter SDK",
    version: languageVersion(
      flutter.value.loading,
      flutter.value.error,
      flutter.value.data?.version?.dartSdkVersion,
    ),
    env: "Flutter",
    to: "/environment/flutter",
  },
  {
    name: "Java",
    detail: "Android JDK",
    version: languageVersion(
      android.value.loading,
      android.value.error,
      android.value.data?.java?.version,
    ),
    env: "Android",
    to: "/environment/android",
  },
  {
    name: "Swift",
    detail: "Xcode / CLT",
    version: languageVersion(
      ios.value.loading,
      ios.value.error,
      ios.value.data?.swiftVersion,
    ),
    env: "iOS / Xcode",
    to: "/environment/ios",
  },
  {
    name: "Python",
    detail: "python3",
    version: toolchainVersion("python"),
    env: "常用工具链",
    to: "/environment/toolchains",
  },
  {
    name: "Node.js",
    detail: "node",
    version: toolchainVersion("node"),
    env: "常用工具链",
    to: "/environment/toolchains",
  },
  {
    name: "Rust",
    detail: "rustc",
    version: toolchainVersion("rust"),
    env: "常用工具链",
    to: "/environment/toolchains",
  },
]);
</script>

<template>
  <section class="page">
    <header class="page-header">
      <div class="page-heading">
        <p class="eyebrow">DevScope</p>
        <h1>总览</h1>
        <p class="subtitle">
          本机配置与常用开发环境一览。点击环境或语言可进入左侧对应的检测详情。
        </p>
      </div>

      <button class="refresh" type="button" :disabled="loading" @click="refresh(true)">
        <span v-if="loading" class="spinner spinner--small" aria-hidden="true"></span>
        {{ loading ? "检测中…" : "刷新" }}
      </button>
    </header>

    <div class="card system-card">
      <div class="card-head">
        <div>
          <h2>本机配置</h2>
          <p class="card-subtitle">{{ systemSubtitle }}</p>
        </div>
        <span v-if="system.loading" class="spinner spinner--small" aria-hidden="true"></span>
      </div>

      <div v-if="system.loading" class="empty-note">正在读取 macOS 版本、芯片与内存信息…</div>
      <div v-else-if="system.error" class="callout callout--error" role="alert">
        {{ system.error }}
      </div>
      <div v-else class="system-grid">
        <div v-for="metric in systemMetrics" :key="metric.label" class="system-item">
          <span class="system-label">{{ metric.label }}</span>
          <span class="system-value">{{ metric.value }}</span>
        </div>
      </div>
    </div>

    <section class="page-section">
      <div class="section-head">
        <div>
          <h2>常用开发环境</h2>
          <p>点击进入左侧对应的环境检测子项。</p>
        </div>
        <StatusBadge :tone="envStatus.tone" :label="envStatus.label" />
      </div>

      <div class="env-list">
        <EnvironmentRow
          title="Flutter"
          monogram="F"
          :tone="statusTone(flutter.data?.status)"
          :status-label="statusLabel(flutter.data?.status)"
          :headline="flutterHeadline()"
          :loading="flutter.loading"
          :error="flutter.error"
          to="/environment/flutter"
        />
        <EnvironmentRow
          title="Android"
          monogram="A"
          :tone="statusTone(android.data?.status)"
          :status-label="statusLabel(android.data?.status)"
          :headline="androidHeadline()"
          :meta="issuesMeta(android.data?.issues)"
          :loading="android.loading"
          :error="android.error"
          to="/environment/android"
        />
        <EnvironmentRow
          title="iOS / Xcode"
          monogram="i"
          :tone="statusTone(ios.data?.status)"
          :status-label="statusLabel(ios.data?.status)"
          :headline="iosHeadline()"
          :meta="issuesMeta(ios.data?.issues)"
          :loading="ios.loading"
          :error="ios.error"
          to="/environment/ios"
        />
        <EnvironmentRow
          title="常用工具链"
          monogram="工"
          :tone="toolchainBadge.tone"
          :status-label="toolchainBadge.label"
          :headline="toolchainHeadline"
          :loading="toolchains.loading"
          :error="toolchains.error"
          to="/environment/toolchains"
        />
      </div>
    </section>

    <section class="page-section">
      <div class="section-head">
        <div>
          <h2>常用语言</h2>
          <p>版本来自对应工具链，点击可查看完整环境。</p>
        </div>
      </div>

      <div class="language-grid">
        <RouterLink
          v-for="language in languages"
          :key="language.name"
          class="language-card"
          :to="language.to"
        >
          <span class="language-head">
            <span class="language-name">{{ language.name }}</span>
            <span class="language-from">{{ language.detail }}</span>
          </span>
          <strong class="language-version">{{ language.version }}</strong>
          <span class="language-link">查看 {{ language.env }} →</span>
        </RouterLink>
      </div>
    </section>
  </section>
</template>

<style scoped>
.system-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 10px;
  margin-top: 18px;
}

.system-item {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 12px 14px;
  border: 1px solid var(--border);
  border-radius: 10px;
  background: var(--surface-soft);
}

.system-label {
  color: var(--text-tertiary);
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.system-value {
  font-size: 15px;
  font-weight: 650;
  letter-spacing: -0.01em;
  overflow-wrap: anywhere;
}

.page-section {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.section-head {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  gap: 16px;
  padding: 0 2px;
}

.section-head h2 {
  margin: 0;
  font-size: 16px;
  font-weight: 650;
  letter-spacing: -0.01em;
}

.section-head p {
  margin: 4px 0 0;
  color: var(--text-secondary);
  font-size: 12px;
}

.env-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.language-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 12px;
}

.language-card {
  display: flex;
  flex-direction: column;
  gap: 7px;
  padding: 16px;
  border: 1px solid var(--border);
  border-radius: 12px;
  background: var(--surface);
  color: var(--text);
  box-shadow: var(--shadow-card);
  transition:
    border-color 0.15s ease,
    background-color 0.15s ease;
}

.language-card:hover {
  border-color: var(--border-strong);
  background: var(--surface-hover);
  text-decoration: none;
}

.language-head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 8px;
}

.language-name {
  font-size: 13px;
  font-weight: 650;
}

.language-from {
  color: var(--text-tertiary);
  font-size: 10.5px;
}

.language-version {
  font-size: 21px;
  font-weight: 700;
  letter-spacing: -0.02em;
  overflow-wrap: anywhere;
}

.language-link {
  color: var(--accent);
  font-size: 11px;
  font-weight: 600;
}

@media (max-width: 760px) {
  .system-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .language-grid {
    grid-template-columns: 1fr;
  }
}

@media (max-width: 560px) {
  .system-grid {
    grid-template-columns: 1fr;
  }
}
</style>
