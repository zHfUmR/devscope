<script setup lang="ts">
/**
 * Flutter 项目分析页。
 *
 * 用户通过原生目录选择器指定项目，Rust 读取 pubspec.yaml、.metadata、
 * Android Gradle 与 iOS 工程配置。页面只处理状态和展示，不直接访问文件。
 *
 * 四种状态：loading / IPC 失败 / 未选择或分析失败（空状态）/ 正常。
 */
import { computed, onMounted, ref } from "vue";
import DetailList from "../components/DetailList.vue";
import IssueList from "../components/IssueList.vue";
import PathActions from "../components/PathActions.vue";
import StatusBadge from "../components/StatusBadge.vue";
import { pickDirectory } from "../services/dialog";
import { toUserMessage } from "../services/errors";
import { analyzeFlutterProject, checkFlutterOutdated } from "../services/rust-api";
import { statusTone } from "../services/status";
import type {
  DependencySource,
  OutdatedReport,
  PlatformKind,
  ProjectAnalysis,
  ProjectDependency,
  ProjectStatus,
} from "../types";

type ViewState = "idle" | "loading" | "ipcError" | "ok" | "error";

interface DetailRow {
  label: string;
  value: string;
  mono?: boolean;
}

interface Metric {
  label: string;
  value: string;
}

interface DependencyGroup {
  label: string;
  items: ProjectDependency[];
}

const STORAGE_KEY = "devscope.project.path";

const PLATFORM_LABELS: Record<PlatformKind, string> = {
  android: "Android",
  ios: "iOS",
  web: "Web",
  macos: "macOS",
  linux: "Linux",
  windows: "Windows",
};

const SOURCE_LABELS: Record<DependencySource, string> = {
  hosted: "pub.dev",
  sdk: "SDK",
  git: "Git",
  path: "本地路径",
  unknown: "未知",
};

const selectedPath = ref<string | null>(null);
const analysis = ref<ProjectAnalysis | null>(null);
const loading = ref(false);
const ipcError = ref<string | null>(null);

const outdated = ref<OutdatedReport | null>(null);
const outdatedLoading = ref(false);
const outdatedError = ref<string | null>(null);

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
    { label: "项目版本", value: data.version ?? "未声明" },
    { label: "Dart SDK", value: data.dartSdkConstraint ?? "未声明" },
    { label: "Flutter SDK", value: data.flutterConstraint ?? "未声明" },
    { label: "依赖", value: `${data.dependencies.length} 个` },
  ];
});

const platformNames = computed<string[]>(
  () => analysis.value?.platforms.map((platform) => PLATFORM_LABELS[platform.platform]) ?? [],
);

const androidRows = computed<DetailRow[]>(() => {
  const config = analysis.value?.android;
  if (!config) {
    return [];
  }

  return [
    { label: "Gradle 文件", value: config.gradleFile, mono: true },
    { label: "DSL", value: config.kotlinDsl ? "Kotlin DSL" : "Groovy" },
    { label: "applicationId", value: config.applicationId ?? "未找到", mono: true },
    { label: "namespace", value: config.namespace ?? "未找到", mono: true },
    { label: "compileSdk", value: config.compileSdk ?? "未声明" },
    { label: "minSdk", value: config.minSdk ?? "未声明" },
    { label: "targetSdk", value: config.targetSdk ?? "未声明" },
  ];
});

const iosRows = computed<DetailRow[]>(() => {
  const config = analysis.value?.ios;
  if (!config) {
    return [];
  }

  return [
    { label: "Bundle Identifier", value: config.bundleIdentifier ?? "未找到", mono: true },
    { label: "部署目标", value: config.deploymentTarget ?? "未声明" },
    { label: "Podfile", value: config.podfilePresent ? "存在" : "缺失" },
    { label: "Xcode Workspace", value: config.workspacePresent ? "存在" : "缺失" },
    { label: "Info.plist", value: config.infoPlistPresent ? "存在" : "缺失" },
  ];
});

const resourceRows = computed<DetailRow[]>(() => {
  const data = analysis.value;
  if (!data) {
    return [];
  }

  return [
    { label: "Material 设计", value: data.usesMaterialDesign ? "已启用" : "未启用" },
    { label: "资源条目", value: `${data.assetCount} 项` },
    {
      label: "字体家族",
      value: data.fontFamilies.length > 0 ? data.fontFamilies.join("、") : "未声明",
    },
    { label: "publish_to", value: data.publishTo ?? "未声明" },
  ];
});

const metadataRows = computed<DetailRow[]>(() => {
  const metadata = analysis.value?.metadata;
  if (!metadata) {
    return [];
  }

  return [
    { label: "Flutter 渠道", value: metadata.channel ?? "未知" },
    { label: "Framework Revision", value: metadata.revision ?? "未知", mono: true },
    { label: "项目类型", value: metadata.projectType ?? "未知" },
  ];
});

const dependencyQuery = ref("");

const totalDependencyCount = computed(() => {
  const data = analysis.value;
  if (!data) {
    return 0;
  }

  return (
    data.dependencies.length +
    data.devDependencies.length +
    data.dependencyOverrides.length
  );
});

const dependencyGroups = computed<DependencyGroup[]>(() => {
  const data = analysis.value;
  if (!data) {
    return [];
  }

  const keyword = dependencyQuery.value.trim().toLowerCase();
  const groups = [
    { label: "dependencies", items: data.dependencies },
    { label: "dev_dependencies", items: data.devDependencies },
    { label: "dependency_overrides", items: data.dependencyOverrides },
  ];

  return groups
    .map((group) => ({
      label: group.label,
      items: keyword
        ? group.items.filter((dependency) =>
            `${dependency.name} ${dependency.constraint ?? ""}`
              .toLowerCase()
              .includes(keyword),
          )
        : group.items,
    }))
    .filter((group) => group.items.length > 0);
});

const issues = computed<string[]>(() => analysis.value?.issues ?? []);

const errorMessage = computed(
  () => analysis.value?.message ?? "无法读取项目配置，请确认目录是否正确。",
);

function projectStatusLabel(status: ProjectStatus): string {
  if (status === "ok") {
    return "结构完整";
  }
  if (status === "warning") {
    return "需注意";
  }

  return "无法分析";
}

function readRememberedPath(): string | null {
  try {
    return localStorage.getItem(STORAGE_KEY);
  } catch {
    return null;
  }
}

function rememberPath(path: string): void {
  try {
    localStorage.setItem(STORAGE_KEY, path);
  } catch {
    // 存储失败只影响下次自动恢复，不影响本次分析。
  }
}

async function runAnalysis(path: string): Promise<void> {
  loading.value = true;
  ipcError.value = null;
  analysis.value = null;
  dependencyQuery.value = "";
  outdated.value = null;
  outdatedError.value = null;

  try {
    const result = await analyzeFlutterProject(path);
    analysis.value = result;
    if (result.status !== "error") {
      rememberPath(path);
    }
  } catch (error) {
    ipcError.value = toUserMessage(error, "分析项目失败，请查看应用日志。");
  } finally {
    loading.value = false;
  }
}

async function chooseProject(): Promise<void> {
  try {
    const path = await pickDirectory("选择 Flutter 项目目录");
    if (!path) {
      return;
    }

    selectedPath.value = path;
    await runAnalysis(path);
  } catch (error) {
    ipcError.value = toUserMessage(error, "无法打开目录选择器。");
  }
}

async function reanalyze(): Promise<void> {
  if (!selectedPath.value || loading.value) {
    return;
  }

  await runAnalysis(selectedPath.value);
}

async function checkOutdated(): Promise<void> {
  if (!analysis.value || outdatedLoading.value) {
    return;
  }

  outdatedLoading.value = true;
  outdatedError.value = null;
  outdated.value = null;

  try {
    outdated.value = await checkFlutterOutdated(analysis.value.rootPath);
  } catch (error) {
    outdatedError.value = toUserMessage(error, "检查依赖更新失败，请查看应用日志。");
  } finally {
    outdatedLoading.value = false;
  }
}

function outdatedBadge(report: OutdatedReport): { tone: "ok" | "warning" | "error"; label: string } {
  if (report.status === "ok") {
    return { tone: "ok", label: "已是最新" };
  }
  if (report.status === "error") {
    return { tone: "error", label: "检查失败" };
  }

  return { tone: "warning", label: `${report.packages.length} 个可更新` };
}

function versionText(version: string | null): string {
  return version ?? "—";
}

onMounted(() => {
  const remembered = readRememberedPath();
  if (!remembered) {
    return;
  }

  selectedPath.value = remembered;
  void runAnalysis(remembered);
});
</script>

<template>
  <section class="page">
    <header class="page-header">
      <div class="page-heading">
        <p class="eyebrow">项目分析</p>
        <h1>Flutter 项目</h1>
        <p class="subtitle">
          选择项目目录，读取 pubspec.yaml、依赖、平台配置和资源声明。分析只读，不会修改项目。
        </p>
      </div>

      <div class="header-actions">
        <button
          class="refresh refresh--primary"
          type="button"
          :disabled="loading"
          @click="chooseProject"
        >
          {{ analysis ? "更换项目" : "选择项目" }}
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
      <span class="path-label">当前目录</span>
      <code>{{ selectedPath }}</code>
      <PathActions :path="selectedPath" />
    </div>

    <div v-if="viewState === 'loading'" class="card state-card" role="status" aria-live="polite">
      <span class="spinner" aria-hidden="true"></span>
      <div>
        <h2>正在读取项目配置…</h2>
        <p>解析 pubspec.yaml、Android Gradle 和 iOS 工程文件。</p>
      </div>
    </div>

    <div
      v-else-if="viewState === 'ipcError'"
      class="card state-card state-card--error"
      role="alert"
    >
      <div class="state-icon state-icon--error" aria-hidden="true">!</div>
      <div class="state-body">
        <h2>无法完成项目分析</h2>
        <p>{{ ipcError }}</p>
        <p class="hint">
          如果你是在浏览器中直接打开页面，请改用 <code>npm run tauri dev</code> 启动桌面应用。
        </p>
        <div class="state-actions">
          <button class="refresh refresh--primary" type="button" @click="chooseProject">
            重新选择目录
          </button>
        </div>
      </div>
    </div>

    <div v-else-if="viewState === 'idle'" class="card state-card">
      <div class="state-icon state-icon--empty" aria-hidden="true">项</div>
      <div class="state-body">
        <h2>选择一个 Flutter 项目</h2>
        <p>
          会读取 <code>pubspec.yaml</code>、<code>.metadata</code>、Android Gradle 文件和
          iOS 工程配置。没有 <code>pubspec.yaml</code> 的目录会被判定为非 Flutter 项目。
        </p>
        <div class="state-actions">
          <button class="refresh refresh--primary" type="button" @click="chooseProject">
            选择项目目录
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
          <h2>无法分析该项目</h2>
          <StatusBadge tone="error" label="分析失败" />
        </div>
        <p>{{ errorMessage }}</p>
        <p class="hint">请选择包含 <code>pubspec.yaml</code> 的 Flutter 项目目录。</p>
        <div class="state-actions">
          <button class="refresh refresh--primary" type="button" @click="chooseProject">
            重新选择目录
          </button>
        </div>
      </div>
    </div>

    <template v-else-if="analysis">
      <div class="card">
        <div class="card-head">
          <div>
            <h2>{{ analysis.name ?? "未命名项目" }}</h2>
            <p class="card-subtitle">
              {{ analysis.description ?? "pubspec.yaml 没有提供项目描述。" }}
            </p>
          </div>
          <StatusBadge
            :tone="statusTone(analysis.status)"
            :label="projectStatusLabel(analysis.status)"
          />
        </div>

        <div class="metrics">
          <div v-for="metric in metrics" :key="metric.label" class="metric">
            <span class="metric-label">{{ metric.label }}</span>
            <span class="metric-value">{{ metric.value }}</span>
          </div>
        </div>

        <div class="platform-row">
          <span class="platform-label">平台</span>
          <span v-for="platform in platformNames" :key="platform" class="tag">
            {{ platform }}
          </span>
          <span v-if="platformNames.length === 0" class="hint">未启用任何平台</span>
        </div>
      </div>

      <div v-if="issues.length > 0" class="card issues-card">
        <div class="card-head">
          <div>
            <h2>需要关注</h2>
            <p class="card-subtitle">{{ issues.length }} 项配置缺失或需要注意。</p>
          </div>
          <StatusBadge tone="warning" :label="`${issues.length} 项`" />
        </div>
        <IssueList :issues="issues" />
      </div>

      <div class="grid-2">
        <div v-if="analysis.android" class="card">
          <h2 class="section-title">Android 配置</h2>
          <DetailList :rows="androidRows" />
        </div>
        <div v-if="analysis.ios" class="card">
          <h2 class="section-title">iOS 配置</h2>
          <DetailList :rows="iosRows" />
        </div>
      </div>

      <div class="card">
        <div class="card-head">
          <div>
            <h2>依赖</h2>
            <p class="card-subtitle">
              常规依赖 {{ analysis.dependencies.length }} 个，开发依赖
              {{ analysis.devDependencies.length }} 个。
            </p>
          </div>
          <input
            v-model="dependencyQuery"
            class="dep-filter"
            type="search"
            placeholder="筛选依赖…"
            aria-label="筛选依赖"
          />
        </div>

        <div v-if="totalDependencyCount === 0" class="empty-note">
          pubspec.yaml 中没有声明依赖。
        </div>
        <div v-else-if="dependencyGroups.length === 0" class="empty-note">
          没有匹配「{{ dependencyQuery }}」的依赖。
        </div>

        <div v-else class="dep-groups">
          <div v-for="group in dependencyGroups" :key="group.label" class="dep-group">
            <div class="dep-group-title">
              <span>{{ group.label }}</span>
              <span class="dep-count">{{ group.items.length }}</span>
            </div>

            <ul class="dep-list">
              <li v-for="dependency in group.items" :key="dependency.name" class="dep-item">
                <span class="dep-name">{{ dependency.name }}</span>
                <span class="dep-constraint">{{ dependency.constraint ?? "any" }}</span>
                <span class="dep-source">{{ SOURCE_LABELS[dependency.source] }}</span>
              </li>
            </ul>
          </div>
        </div>
      </div>

      <div class="card">
        <div class="card-head">
          <div>
            <h2>依赖更新</h2>
            <p class="card-subtitle">
              运行 flutter pub outdated --json 检查 pub.dev 上的新版本。需要联网，不会修改 pubspec.yaml。
            </p>
          </div>
          <button
            class="refresh refresh--primary"
            type="button"
            :disabled="outdatedLoading"
            @click="checkOutdated"
          >
            <span
              v-if="outdatedLoading"
              class="spinner spinner--small"
              aria-hidden="true"
            ></span>
            {{ outdatedLoading ? "检查中…" : outdated ? "重新检查" : "检查依赖更新" }}
          </button>
        </div>

        <div v-if="outdatedError" class="callout callout--error" role="alert">
          {{ outdatedError }}
        </div>
        <div v-else-if="outdatedLoading" class="empty-note">
          正在解析依赖并访问 pub.dev，首次检查可能需要一段时间…
        </div>
        <template v-else-if="outdated">
          <div class="outdated-head">
            <StatusBadge
              :tone="outdatedBadge(outdated).tone"
              :label="outdatedBadge(outdated).label"
            />
            <span v-if="outdated.message" class="outdated-note">{{ outdated.message }}</span>
          </div>

          <div v-if="outdated.status === 'error'" class="callout callout--error" role="alert">
            {{ outdated.message ?? "检查依赖更新失败。" }}
          </div>
          <div v-else-if="outdated.packages.length === 0" class="empty-note">
            所有依赖都已是最新版本。
          </div>
          <div v-else class="table-wrap">
            <table class="outdated-table">
              <thead>
                <tr>
                  <th>包名</th>
                  <th>当前</th>
                  <th>可升级</th>
                  <th>可解析</th>
                  <th>最新</th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="packageItem in outdated.packages" :key="packageItem.name">
                  <td>{{ packageItem.name }}</td>
                  <td class="mono">{{ versionText(packageItem.current) }}</td>
                  <td class="mono">{{ versionText(packageItem.upgradable) }}</td>
                  <td class="mono">{{ versionText(packageItem.resolvable) }}</td>
                  <td class="mono outdated-latest">
                    {{ versionText(packageItem.latest) }}
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
        </template>
        <div v-else class="empty-note">
          点击「检查依赖更新」运行 flutter pub outdated。该操作会访问 pub.dev，可能需要数十秒。
        </div>
      </div>

      <div class="grid-2">
        <div class="card">
          <h2 class="section-title">Flutter 资源</h2>
          <DetailList :rows="resourceRows" />

          <details v-if="analysis.assets.length > 0" class="checked">
            <summary>查看 {{ analysis.assets.length }} 项资源声明</summary>
            <ul>
              <li v-for="asset in analysis.assets" :key="asset">
                <code>{{ asset }}</code>
              </li>
            </ul>
          </details>
        </div>
        <div v-if="metadataRows.length > 0" class="card">
          <h2 class="section-title">项目元数据</h2>
          <DetailList :rows="metadataRows" />
        </div>
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
  background: transparent;
  padding: 0;
  color: var(--text-secondary);
  font-size: 11.5px;
}

.platform-row {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 6px;
  margin-top: 16px;
}

.platform-label {
  margin-right: 2px;
  color: var(--text-secondary);
  font-size: 12px;
}

.state-actions {
  margin-top: 14px;
}

.outdated-head {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 10px;
  margin-top: 14px;
}

.outdated-note {
  color: var(--text-secondary);
  font-size: 11.5px;
}

.outdated-table {
  width: 100%;
  margin-top: 14px;
  border-collapse: collapse;
  font-size: 11.5px;
}

.outdated-table th {
  padding: 0 6px 7px;
  border-bottom: 1px solid var(--border);
  color: var(--text-tertiary);
  font-size: 9.5px;
  font-weight: 700;
  letter-spacing: 0.06em;
  text-align: left;
  text-transform: uppercase;
}

.outdated-table td {
  padding: 8px 6px;
  border-bottom: 1px solid var(--border-soft);
  color: var(--text-secondary);
  overflow-wrap: anywhere;
}

.outdated-table tr:last-child td {
  border-bottom: 0;
}

.outdated-latest {
  color: var(--warning) !important;
  font-weight: 650;
}

.dep-filter {
  width: 180px;
  flex-shrink: 0;
  padding: 6px 9px;
  border: 1px solid var(--border);
  border-radius: 7px;
  outline: none;
  background: var(--surface);
  color: var(--text);
  font: inherit;
  font-size: 11.5px;
}

.dep-filter::placeholder {
  color: var(--text-tertiary);
}

.dep-filter:focus {
  border-color: var(--accent);
  outline: 2px solid var(--accent);
  outline-offset: 1px;
}

.dep-groups {
  display: flex;
  flex-direction: column;
  gap: 16px;
  margin-top: 16px;
}

.dep-group-title {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  color: var(--text-secondary);
  font-family: "SFMono-Regular", ui-monospace, "SF Mono", Menlo, monospace;
  font-size: 11.5px;
  font-weight: 600;
}

.dep-count {
  color: var(--text-tertiary);
}

.dep-list {
  max-height: 240px;
  margin: 8px 0 0;
  padding: 0;
  overflow: auto;
  list-style: none;
  border-top: 1px solid var(--border);
}

.dep-item {
  display: flex;
  align-items: baseline;
  gap: 10px;
  padding: 7px 2px;
  border-bottom: 1px solid var(--border-soft);
}

.dep-name {
  flex-shrink: 0;
  font-size: 12px;
  font-weight: 600;
}

.dep-constraint {
  min-width: 0;
  flex: 1;
  color: var(--text-secondary);
  font-family: "SFMono-Regular", ui-monospace, "SF Mono", Menlo, monospace;
  font-size: 11px;
  overflow-wrap: anywhere;
}

.dep-source {
  flex-shrink: 0;
  padding: 2px 6px;
  border: 1px solid var(--border);
  border-radius: 5px;
  background: var(--surface-soft);
  color: var(--text-tertiary);
  font-size: 9.5px;
  font-weight: 600;
}

.issues-card {
  border-color: var(--warning-border);
  background: var(--warning-surface);
}

@media (max-width: 680px) {
  .dep-filter {
    width: 100%;
  }

  .path-bar {
    align-items: flex-start;
    flex-direction: column;
    gap: 4px;
  }
}
</style>
