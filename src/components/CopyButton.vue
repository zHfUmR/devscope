<script setup lang="ts">
/**
 * 复制按钮：把一段文本写入剪贴板，短暂显示成功/失败反馈。
 */
import { onUnmounted, ref } from "vue";
import { copyText } from "../services/clipboard";
import { toUserMessage } from "../services/errors";

const props = withDefaults(
  defineProps<{
    text: string;
    label?: string;
  }>(),
  { label: "复制" },
);

const state = ref<"idle" | "copied" | "error">("idle");
const message = ref("");
let timer: number | undefined;

function resetLater(): void {
  if (timer !== undefined) {
    window.clearTimeout(timer);
  }
  timer = window.setTimeout(() => {
    state.value = "idle";
    message.value = "";
  }, 2000);
}

async function copy(): Promise<void> {
  try {
    await copyText(props.text);
    state.value = "copied";
    message.value = "已复制";
  } catch (error) {
    state.value = "error";
    message.value = toUserMessage(error, "复制失败。");
  }

  resetLater();
}

onUnmounted(() => {
  if (timer !== undefined) {
    window.clearTimeout(timer);
  }
});
</script>

<template>
  <span class="copy-wrap">
    <button
      class="copy-button"
      :class="{ 'copy-button--error': state === 'error' }"
      type="button"
      @click="copy"
    >
      {{ state === "copied" ? "已复制" : label }}
    </button>
    <span v-if="state === 'error'" class="copy-error" role="alert">{{ message }}</span>
  </span>
</template>

<style scoped>
.copy-wrap {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

.copy-button {
  padding: 2px 7px;
  border: 1px solid var(--border);
  border-radius: 5px;
  background: var(--surface);
  color: var(--text-secondary);
  font-size: 10px;
  font-weight: 600;
  line-height: 1.5;
  cursor: pointer;
}

.copy-button:hover {
  border-color: var(--border-strong);
  background: var(--surface-hover);
  color: var(--text);
}

.copy-button:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 1px;
}

.copy-button--error {
  border-color: var(--error-border);
  color: var(--error);
}

.copy-error {
  color: var(--error);
  font-size: 10px;
}
</style>
