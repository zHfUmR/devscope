<script setup lang="ts">
/**
 * 关于页：版本、开源许可、数据策略和项目链接。
 *
 * macOS 的系统「关于」面板不展示许可证文本，所以这里单独提供一个可见入口。
 */
import { onMounted, ref } from "vue";
import { ISSUES_URL, LICENSE_URL, REPOSITORY_URL, getAppVersion } from "../services/app-info";
import { toUserMessage } from "../services/errors";
import { openExternalUrl } from "../services/reveal";

const version = ref("0.1.0");
const linkError = ref<string | null>(null);

async function openLink(url: string): Promise<void> {
  linkError.value = null;

  try {
    await openExternalUrl(url);
  } catch (error) {
    linkError.value = toUserMessage(error, "无法打开链接。");
  }
}

onMounted(async () => {
  version.value = await getAppVersion();
});
</script>

<template>
  <section class="page">
    <header class="page-header">
      <div class="page-heading">
        <p class="eyebrow">应用</p>
        <h1>关于 DevScope</h1>
        <p class="subtitle">
          macOS 开发者工具箱：本地环境诊断、Flutter 项目分析与构建日志分析。
        </p>
      </div>
    </header>

    <div class="card about-hero">
      <div class="about-brand" aria-hidden="true">DS</div>
      <div class="about-copy">
        <div class="about-title">
          <h2>DevScope</h2>
          <span class="version">v{{ version }}</span>
        </div>
        <p class="card-subtitle">
          所有检测都在本机完成，不需要账号，也不会上传项目或日志内容。
        </p>
        <div class="tag-list about-tags">
          <span class="tag">Tauri 2</span>
          <span class="tag">Rust</span>
          <span class="tag">Vue 3</span>
          <span class="tag">TypeScript</span>
        </div>
      </div>
    </div>

    <div class="card">
      <div class="card-head">
        <div>
          <h2>开源许可</h2>
          <p class="card-subtitle">MIT License · Copyright (c) 2026 DevScope</p>
        </div>
        <button class="refresh" type="button" @click="openLink(LICENSE_URL)">
          查看许可证全文
        </button>
      </div>

      <p class="license-summary">
        你可以自由使用、复制、修改、合并、发布、分发、再许可或销售本软件，
        但需要在所有副本或重要部分中保留版权声明和许可声明。软件按「现状」提供，
        不附带任何明示或暗示的担保。
      </p>

      <ul class="license-points">
        <li>允许商业使用、修改和再分发</li>
        <li>必须保留版权与许可声明</li>
        <li>作者不对使用后果承担担保责任</li>
      </ul>
    </div>

    <div class="card">
      <h2 class="section-title">项目</h2>
      <div class="about-actions">
        <button class="refresh" type="button" @click="openLink(REPOSITORY_URL)">
          项目主页
        </button>
        <button class="refresh" type="button" @click="openLink(ISSUES_URL)">
          问题反馈
        </button>
      </div>

      <div v-if="linkError" class="callout callout--error" role="alert">
        {{ linkError }}
      </div>

      <dl class="details about-details">
        <div class="detail-row">
          <dt>仓库</dt>
          <dd class="mono">{{ REPOSITORY_URL }}</dd>
        </div>
        <div class="detail-row">
          <dt>许可证</dt>
          <dd>MIT</dd>
        </div>
        <div class="detail-row">
          <dt>目标平台</dt>
          <dd>macOS 12+ · Apple Silicon</dd>
        </div>
      </dl>
    </div>

    <div class="card">
      <h2 class="section-title">数据与隐私</h2>
      <p class="privacy-copy">
        DevScope 只读取本机开发环境、项目配置和用户主动选择的日志文件。
        环境检测、项目分析、日志分析和报告导出都在本地完成；
        只有「检查依赖更新」会按需访问 pub.dev，且必须由你手动触发。
      </p>
    </div>
  </section>
</template>

<style scoped>
.about-hero {
  display: flex;
  align-items: flex-start;
  gap: 16px;
}

.about-brand {
  display: grid;
  place-items: center;
  flex-shrink: 0;
  width: 54px;
  height: 54px;
  border-radius: 15px;
  background: linear-gradient(145deg, #0071e3, #42a5f5);
  color: var(--on-brand);
  font-size: 17px;
  font-weight: 700;
  box-shadow: var(--shadow-accent);
}

.about-copy {
  min-width: 0;
  flex: 1;
}

.about-title {
  display: flex;
  align-items: baseline;
  gap: 8px;
}

.about-title h2 {
  margin: 0;
}

.version {
  color: var(--text-tertiary);
  font-family: "SFMono-Regular", ui-monospace, "SF Mono", Menlo, monospace;
  font-size: 11px;
}

.about-tags {
  margin-top: 12px;
}

.license-summary {
  margin: 14px 0 0;
  color: var(--text-secondary);
  font-size: 12.5px;
  line-height: 1.7;
}

.license-points {
  margin: 12px 0 0;
  padding: 0;
  list-style: none;
}

.license-points li {
  position: relative;
  padding: 6px 0 6px 14px;
  color: var(--text-secondary);
  font-size: 12px;
}

.license-points li::before {
  position: absolute;
  top: 12px;
  left: 1px;
  width: 4px;
  height: 4px;
  border-radius: 50%;
  background: var(--text-tertiary);
  content: "";
}

.about-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.about-details {
  margin-top: 16px;
}

.privacy-copy {
  margin: 0;
  color: var(--text-secondary);
  font-size: 12.5px;
  line-height: 1.7;
}

@media (max-width: 560px) {
  .about-hero {
    flex-direction: column;
  }
}
</style>
