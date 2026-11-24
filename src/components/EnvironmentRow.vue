<script setup lang="ts">
/**
 * 总览页的开发环境行。
 *
 * 点击整行进入对应环境检测页；加载、IPC 失败和正常数据三种形态由父组件切换。
 */
import { RouterLink } from "vue-router";
import StatusBadge from "./StatusBadge.vue";

type Tone = "ok" | "warning" | "error";

defineProps<{
  title: string;
  /** 左侧单字母标记，例如 F / A / i。 */
  monogram: string;
  tone: Tone;
  statusLabel: string;
  headline: string;
  meta?: string;
  to: string;
  loading?: boolean;
  error?: string | null;
}>();
</script>

<template>
  <RouterLink
    class="env-row"
    :class="{ 'env-row--muted': loading || error }"
    :to="to"
  >
    <span class="env-monogram" aria-hidden="true">{{ monogram }}</span>
    <span class="env-copy">
      <span class="env-title">{{ title }}</span>
      <span class="env-headline" :title="error ?? headline">
        {{ loading ? "正在检测…" : (error ?? headline) }}
      </span>
    </span>

    <span v-if="loading" class="spinner spinner--small" aria-hidden="true"></span>
    <span v-else class="env-side">
      <span v-if="meta && !error" class="env-meta">{{ meta }}</span>
      <StatusBadge v-if="error" tone="error" label="调用失败" />
      <StatusBadge v-else :tone="tone" :label="statusLabel" />
    </span>
    <span v-if="!loading" class="env-arrow" aria-hidden="true">→</span>
  </RouterLink>
</template>

<style scoped>
.env-row {
  display: flex;
  align-items: center;
  gap: 13px;
  padding: 13px 15px;
  border: 1px solid var(--border);
  border-radius: 12px;
  background: var(--surface);
  color: var(--text);
  box-shadow: var(--shadow-card);
  transition:
    border-color 0.15s ease,
    background-color 0.15s ease,
    transform 0.15s ease;
}

.env-row:hover {
  border-color: var(--border-strong);
  background: var(--surface-hover);
  text-decoration: none;
}

.env-row:active {
  transform: translateY(1px);
}

.env-row--muted {
  background: var(--surface-soft);
}

.env-monogram {
  display: grid;
  place-items: center;
  flex-shrink: 0;
  width: 28px;
  height: 28px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--surface-soft);
  color: var(--text-secondary);
  font-size: 11px;
  font-weight: 700;
}

.env-copy {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.env-title {
  font-size: 13px;
  font-weight: 650;
}

.env-headline {
  overflow: hidden;
  color: var(--text-secondary);
  font-size: 11.5px;
  line-height: 1.5;
  overflow-wrap: anywhere;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.env-side {
  display: flex;
  align-items: center;
  flex-shrink: 0;
  gap: 10px;
}

.env-meta {
  color: var(--warning);
  font-size: 11px;
  white-space: nowrap;
}

.env-arrow {
  flex-shrink: 0;
  color: var(--text-tertiary);
  font-size: 13px;
}

@media (max-width: 560px) {
  .env-meta {
    display: none;
  }

  .env-headline {
    white-space: normal;
  }
}
</style>
