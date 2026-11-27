<script setup lang="ts">
/**
 * 通用详情行列表。
 *
 * 页面把结构化数据整理成 `{ label, value, mono }`，这里只负责 <dl> 排版，
 * 不关心业务含义；样式复用 app.css 里的 `.details` / `.detail-row`。
 */
import type { DetailRow } from "../types/ui";

defineProps<{
  rows: DetailRow[];
}>();
</script>

<template>
  <dl v-if="rows.length > 0" class="details">
    <div v-for="row in rows" :key="row.label" class="detail-row">
      <dt>{{ row.label }}</dt>
      <dd :class="{ mono: row.mono }">{{ row.value }}</dd>
    </div>
  </dl>
</template>
