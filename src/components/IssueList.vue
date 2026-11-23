<script setup lang="ts">
/**
 * 环境问题列表。
 *
 * Android / iOS 详情页都会把 `issues` 原样交给这个组件；状态颜色由调用方决定，
 * 组件只负责列表语义和排版。
 */
defineProps<{
  issues: string[];
}>();
</script>

<template>
  <ul v-if="issues.length > 0" class="issues">
    <li v-for="issue in issues" :key="issue">{{ issue }}</li>
  </ul>
</template>

<style scoped>
.issues {
  margin: 12px 0 0;
  padding: 0;
  list-style: none;
}

.issues li {
  position: relative;
  padding: 7px 0 7px 16px;
  border-top: 1px solid var(--border);
  color: var(--text-secondary);
  font-size: 12px;
  line-height: 1.55;
  overflow-wrap: anywhere;
}

.issues li:first-child {
  border-top: 0;
  padding-top: 0;
}

.issues li::before {
  position: absolute;
  top: 13px;
  left: 1px;
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background: var(--warning);
  content: "";
}

.issues li:first-child::before {
  top: 6px;
}
</style>
