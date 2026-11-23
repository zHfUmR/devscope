<script setup lang="ts">
/**
 * 通用状态标签。
 *
 * 只负责展示，不关心业务含义；调用方决定 tone 和 label。
 */
type Tone = "ok" | "warning" | "error";

defineProps<{
  tone: Tone;
  label: string;
}>();
</script>

<template>
  <span class="badge" :class="`badge--${tone}`">
    <span class="badge-dot" aria-hidden="true"></span>
    {{ label }}
  </span>
</template>

<style scoped>
.badge {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 4px 10px;
  border-radius: 999px;
  font-size: 12px;
  font-weight: 600;
  line-height: 1.4;
  white-space: nowrap;
}

.badge-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: currentColor;
}

.badge--ok {
  color: var(--ok);
  background: var(--ok-soft);
}

.badge--warning {
  color: var(--warning);
  background: var(--warning-soft);
}

.badge--error {
  color: var(--error);
  background: var(--error-soft);
}
</style>
