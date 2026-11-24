<script setup lang="ts">
/**
 * iOS / Xcode 环境检测页。
 *
 * 与 Flutter、Android 使用同一套状态模型；Xcode 授权、CocoaPods 和模拟器
 * 的项目各自展示，doctor 摘要只负责跳转，不在这里重复 Flutter 的判断。
 */
import { computed, onMounted, ref } from "vue";
import IssueList from "../components/IssueList.vue";
import StatusBadge from "../components/StatusBadge.vue";
import { detectIosEnvironment } from "../services/rust-api";
import { statusLabel, statusTone } from "../services/status";
import type { CocoaPodsSource, IosEnvironment } from "../types";

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

const COCOAPODS_SOURCE_LABELS: Record<CocoaPodsSource, string> = {
  path: "PATH",
  commonLocation: "常见安装目录",
  loginShell: "登录 Shell",
};

const environment = ref<IosEnvironment | null>(null);
const loading = ref(false);
const started = ref(false);
const ipcError = ref<string | null>(null);

/** 第一次加载完成前也显示 loading。 */
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

  const xcode = data.xcodeVersion ?? (data.xcodePath ? "版本未知" : "未安装");
  const cocoapods = data.cocoapods
    ? data.cocoapods.version ?? "版本未知"
    : "未安装";
  const simulators = data.simulators
    ? `${data.simulators.availableCount} 台`
    : "无法读取";

  return [
    { label: "Xcode", value: xcode },
    { label: "CocoaPods", value: cocoapods },
    { label: "iOS 模拟器", value: simulators },
  ];
});

const xcodeDetails = computed<DetailRow[]>(() => {
  const data = environment.value;
  const rows: DetailRow[] = [];

  if (!data) {
    return rows;
  }
  if (data.developerDir) {
    rows.push({ label: "开发者目录", value: data.developerDir, mono: true });
  }
  if (data.xcodePath) {
    rows.push({ label: "Xcode.app", value: data.xcodePath, mono: true });
  }
  if (data.xcodeBuild) {
    rows.push({ label: "Build", value: data.xcodeBuild });
  }
  if (data.commandLineToolsPath) {
    rows.push({ label: "Command Line Tools", value: data.commandLineToolsPath, mono: true });
  }

  return rows;
});

const cocoapodsDetails = computed<DetailRow[]>(() => {
  const data = environment.value?.cocoapods;
  if (!data) {
    return [];
  }

  return [
    { label: "pod 路径", value: data.path, mono: true },
    { label: "版本", value: data.version ?? "无法读取" },
    { label: "发现方式", value: COCOAPODS_SOURCE_LABELS[data.source] },
  ];
});

/** 执行一次检测；请求进行中按钮保持禁用。 */
async function refresh(force = false): Promise<void> {
  if (loading.value) {
    return;
  }

  loading.value = true;
  ipcError.value = null;

  try {
    environment.value = await detectIosEnvironment(force);
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
        <h1>iOS / Xcode</h1>
        <p class="subtitle">
          检查 Xcode、Command Line Tools、CocoaPods、Swift 和 iOS 模拟器，不修改系统配置。
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
        <h2>正在检测 iOS 环境…</h2>
        <p>正在读取 Xcode、CocoaPods 和模拟器信息，通常需要几秒。</p>
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
            <h2>Xcode</h2>
            <p class="card-subtitle">
              {{ environment?.status === "ok" ? "iOS 工具链齐全。" : `有 ${issues.length} 项需要注意。` }}
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
          不影响 Xcode 本身的使用，但可能阻断 Flutter iOS 构建或模拟器调试。
        </p>
      </div>

      <div class="grid-2">
        <div class="card">
          <h2>Xcode 详情</h2>
          <p class="card-subtitle">开发者目录、Build 号和 Command Line Tools。</p>

          <dl v-if="xcodeDetails.length > 0" class="details details--spaced">
            <div v-for="row in xcodeDetails" :key="row.label" class="detail-row">
              <dt>{{ row.label }}</dt>
              <dd :class="{ mono: row.mono }">{{ row.value }}</dd>
            </div>
          </dl>
        </div>

        <div class="card">
          <h2>CocoaPods</h2>
          <p class="card-subtitle">Flutter iOS 插件通常依赖 CocoaPods。</p>

          <dl v-if="cocoapodsDetails.length > 0" class="details details--spaced">
            <div v-for="row in cocoapodsDetails" :key="row.label" class="detail-row">
              <dt>{{ row.label }}</dt>
              <dd :class="{ mono: row.mono }">{{ row.value }}</dd>
            </div>
          </dl>
          <div v-else class="empty-note">
            未安装 CocoaPods。可以用 <code>brew install cocoapods</code> 安装，DevScope 不会代替你执行。
          </div>
        </div>
      </div>

      <div class="grid-2">
        <div class="card">
          <h2>iOS 模拟器</h2>
          <p class="card-subtitle">来自 <code>xcrun simctl</code> 的可用设备。</p>

          <template v-if="environment?.simulators">
            <div class="simulator-count">
              <strong>{{ environment.simulators.availableCount }}</strong>
              <span>台可用设备</span>
            </div>
            <div v-if="environment.simulators.runtimes.length" class="tag-list">
              <span
                v-for="runtime in environment.simulators.runtimes"
                :key="runtime"
                class="tag"
              >
                {{ runtime }}
              </span>
            </div>
            <div v-else class="empty-note">
              没有可用的 iOS runtime。可以在 Xcode 的 Settings → Components 中安装。
            </div>
          </template>
          <div v-else class="empty-note">
            {{
              environment?.xcodePath
                ? "无法读取模拟器列表。"
                : "需要完整的 Xcode 才能使用 iOS 模拟器。"
            }}
          </div>
        </div>

        <div class="card">
          <h2>Swift</h2>
          <p class="card-subtitle">Command Line Tools 提供的 Swift 编译器。</p>

          <div v-if="environment?.swiftVersion" class="swift-version">
            Swift {{ environment.swiftVersion }}
          </div>
          <div v-else class="empty-note">未找到 Swift 编译器。</div>
        </div>
      </div>

      <div v-if="issues.length > 0" class="card">
        <div class="card-head">
          <div>
            <h2>需要注意</h2>
            <p class="card-subtitle">按优先级修复后再重新检测。</p>
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
          <h2>未检测到 Xcode</h2>
          <StatusBadge tone="warning" label="未安装" />
        </div>
        <p>{{ failureMessage }}</p>
        <p class="hint">
          可以从 App Store 安装 Xcode，或运行 <code>xcode-select --install</code> 安装命令行工具。
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
          <h2>iOS 检测失败</h2>
          <StatusBadge tone="error" label="检测异常" />
        </div>
        <p>{{ failureMessage }}</p>

        <p v-if="environment?.developerDir" class="hint">
          开发者目录：<code>{{ environment.developerDir }}</code>
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

.simulator-count {
  display: flex;
  align-items: baseline;
  gap: 7px;
  margin-top: 16px;
}

.simulator-count strong {
  font-size: 28px;
  font-weight: 700;
  letter-spacing: -0.03em;
}

.simulator-count span {
  color: var(--text-secondary);
  font-size: 12px;
}

.swift-version {
  margin-top: 18px;
  color: var(--text);
  font-size: 24px;
  font-weight: 650;
  letter-spacing: -0.02em;
}
</style>
