<script setup lang="ts">
/**
 * Android 环境检测页。
 *
 * 对齐 Flutter 页的状态模型：loading / ipcError / notFound / error / ready。
 * ready 同时覆盖 ok 和 warning，warning 时在页面内列出可修复项。
 */
import { computed, onMounted, ref } from "vue";
import IssueList from "../components/IssueList.vue";
import StatusBadge from "../components/StatusBadge.vue";
import { detectAndroidEnvironment } from "../services/rust-api";
import { statusLabel, statusTone } from "../services/status";
import type {
  AndroidEnvironment,
  AndroidSdkSource,
  JavaSource,
} from "../types";

type ViewState = "loading" | "ipcError" | "notFound" | "error" | "ready";

interface Metric {
  label: string;
  value: string;
}

interface DetailRow {
  label: string;
  value: string;
  mono?: boolean;
}

const SDK_SOURCE_LABELS: Record<AndroidSdkSource, string> = {
  environment: "环境变量",
  commonLocation: "常见安装目录",
};

const JAVA_SOURCE_LABELS: Record<JavaSource, string> = {
  javaHome: "JAVA_HOME",
  androidStudio: "Android Studio JBR",
  system: "系统 / Homebrew / SDKMAN",
};

const environment = ref<AndroidEnvironment | null>(null);
const loading = ref(false);
const started = ref(false);
const ipcError = ref<string | null>(null);

/** 第一次加载完成前也显示 loading，避免页面先闪空内容。 */
const viewState = computed<ViewState>(() => {
  if (!started.value || loading.value) {
    return "loading";
  }
  if (ipcError.value) {
    return "ipcError";
  }

  const status = environment.value?.status;
  if (status === "notFound" || status === "error") {
    return status;
  }

  return "ready";
});

const issues = computed(() => environment.value?.issues ?? []);
const checkedPaths = computed(() => environment.value?.checkedPaths ?? []);
const failureMessage = computed(
  () => environment.value?.message ?? "检测失败，但没有返回具体原因。",
);

const metrics = computed<Metric[]>(() => {
  const data = environment.value;
  if (!data) {
    return [];
  }

  return [
    { label: "最新 Platform", value: data.platforms[0] ?? "未安装" },
    { label: "Build Tools", value: data.buildTools[0] ?? "未安装" },
    { label: "Licenses", value: `${data.acceptedLicenses} 项已接受` },
  ];
});

const details = computed<DetailRow[]>(() => {
  const data = environment.value;
  const rows: DetailRow[] = [];

  if (!data) {
    return rows;
  }
  if (data.sdkPath) {
    rows.push({ label: "SDK 路径", value: data.sdkPath, mono: true });
  }
  if (data.sdkSource) {
    rows.push({ label: "发现方式", value: SDK_SOURCE_LABELS[data.sdkSource] });
  }
  if (data.cmdlineToolsVersion) {
    rows.push({ label: "cmdline-tools", value: data.cmdlineToolsVersion });
  }
  if (data.sdkManagerPath) {
    rows.push({ label: "sdkmanager", value: data.sdkManagerPath, mono: true });
  }
  if (data.adbPath) {
    rows.push({ label: "adb", value: data.adbPath, mono: true });
  }
  if (data.emulatorPath) {
    rows.push({ label: "emulator", value: data.emulatorPath, mono: true });
  }

  return rows;
});

const javaDetails = computed<DetailRow[]>(() => {
  const java = environment.value?.java;
  if (!java) {
    return [];
  }

  return [
    { label: "JDK 路径", value: java.path, mono: true },
    { label: "版本", value: java.version ?? "未提供" },
    { label: "发现方式", value: JAVA_SOURCE_LABELS[java.source] },
  ];
});

/** 执行一次检测；请求进行中按钮保持禁用。 */
async function refresh(): Promise<void> {
  if (loading.value) {
    return;
  }

  loading.value = true;
  ipcError.value = null;

  try {
    environment.value = await detectAndroidEnvironment();
  } catch (error) {
    environment.value = null;
    ipcError.value =
      error instanceof Error && error.message.trim()
        ? error.message
        : "调用 Rust 命令失败，请查看应用日志。";
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
        <h1>Android</h1>
        <p class="subtitle">
          检查 Android SDK、JDK、build-tools、licenses 和模拟器，不启动 adb 或 emulator。
        </p>
      </div>

      <button class="refresh" type="button" :disabled="loading" @click="refresh">
        <span v-if="loading" class="spinner spinner--small" aria-hidden="true"></span>
        {{ loading ? "检测中…" : "重新检测" }}
      </button>
    </header>

    <div v-if="viewState === 'loading'" class="card state-card" role="status" aria-live="polite">
      <span class="spinner" aria-hidden="true"></span>
      <div>
        <h2>正在检测 Android 环境…</h2>
        <p>正在读取 SDK 目录和 JDK 信息，请稍候。</p>
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

    <template v-else-if="viewState === 'ready'">
      <div class="card">
        <div class="card-head">
          <div>
            <h2>Android SDK</h2>
            <p class="card-subtitle">
              {{ environment?.status === "ok" ? "SDK 组件齐全。" : `有 ${issues.length} 项需要注意。` }}
            </p>
          </div>
          <StatusBadge :tone="statusTone(environment?.status)" :label="statusLabel(environment?.status)" />
        </div>

        <div class="metrics">
          <div v-for="metric in metrics" :key="metric.label" class="metric">
            <span class="metric-label">{{ metric.label }}</span>
            <span class="metric-value">{{ metric.value }}</span>
          </div>
        </div>

        <p v-if="environment?.status === 'warning'" class="callout callout--warning">
          不影响继续使用，但建议按下方「需要注意」逐项补齐。
        </p>
      </div>

      <div class="grid-2">
        <div class="card">
          <h2>Java 运行时</h2>
          <p class="card-subtitle">Flutter 构建 Android 应用所需的 JDK。</p>

          <dl v-if="javaDetails.length > 0" class="details details--spaced">
            <div v-for="row in javaDetails" :key="row.label" class="detail-row">
              <dt>{{ row.label }}</dt>
              <dd :class="{ mono: row.mono }">{{ row.value }}</dd>
            </div>
          </dl>
          <div v-else class="empty-note">
            未找到 JDK。请安装 Android Studio，或设置 <code>JAVA_HOME</code>。
          </div>
        </div>

        <div class="card">
          <h2>模拟器</h2>
          <p class="card-subtitle">已配置的 Android Virtual Device（AVD）。</p>

          <div v-if="environment?.avds.length" class="tag-list">
            <span v-for="avd in environment.avds" :key="avd" class="tag">{{ avd }}</span>
          </div>
          <div v-else class="empty-note">
            没有配置 AVD。可以在 Android Studio 的 Device Manager 中创建，或直接使用真机。
          </div>

          <p v-if="!environment?.emulatorPath" class="callout callout--warning">
            未找到 emulator 可执行文件，暂时无法启动模拟器。
          </p>
        </div>
      </div>

      <div class="grid-2">
        <div class="card">
          <h2>Platform</h2>
          <p class="card-subtitle">已安装的 Android API level。</p>
          <div v-if="environment?.platforms.length" class="tag-list">
            <span v-for="platform in environment.platforms" :key="platform" class="tag">
              {{ platform }}
            </span>
          </div>
          <div v-else class="empty-note">没有安装任何 Platform。</div>
        </div>

        <div class="card">
          <h2>Build Tools</h2>
          <p class="card-subtitle">编译 Android 应用使用的构建工具版本。</p>
          <div v-if="environment?.buildTools.length" class="tag-list">
            <span v-for="version in environment.buildTools" :key="version" class="tag">
              {{ version }}
            </span>
          </div>
          <div v-else class="empty-note">没有安装 Build Tools。</div>
        </div>
      </div>

      <div class="card">
        <h2 class="section-title">安装详情</h2>
        <dl v-if="details.length > 0" class="details">
          <div v-for="row in details" :key="row.label" class="detail-row">
            <dt>{{ row.label }}</dt>
            <dd :class="{ mono: row.mono }">{{ row.value }}</dd>
          </div>
        </dl>
        <p v-else class="hint">没有可展示的路径信息。</p>
      </div>

      <div v-if="issues.length > 0" class="card">
        <div class="card-head">
          <div>
            <h2>需要注意</h2>
            <p class="card-subtitle">这些项不会中断检测，但会影响完整开发体验。</p>
          </div>
          <StatusBadge tone="warning" :label="`${issues.length} 项`" />
        </div>
        <IssueList :issues="issues" />
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
          <h2>未检测到 Android SDK</h2>
          <StatusBadge tone="warning" label="未安装" />
        </div>
        <p>{{ failureMessage }}</p>
        <p class="hint">
          安装 Android Studio 后，默认 SDK 位于 <code>~/Library/Android/sdk</code>；也可以设置
          <code>ANDROID_HOME</code>。
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
          <h2>Android 检测失败</h2>
          <StatusBadge tone="error" label="检测异常" />
        </div>
        <p>{{ failureMessage }}</p>

        <p v-if="environment?.sdkPath" class="hint">
          检测到的路径：<code>{{ environment.sdkPath }}</code>
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
.details--spaced {
  margin-top: 14px;
}
</style>
