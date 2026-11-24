<script setup lang="ts">
/**
 * 路径操作：复制路径 + 在 Finder 中显示。
 *
 * 只用于确实存在的路径；文件写入仍由 Rust 或系统插件完成。
 */
import { ref } from "vue";
import { toUserMessage } from "../services/errors";
import { revealInFinder } from "../services/reveal";
import CopyButton from "./CopyButton.vue";

const props = withDefaults(
  defineProps<{
    path: string;
    reveal?: boolean;
  }>(),
  { reveal: true },
);

const revealing = ref(false);
const error = ref<string | null>(null);

async function reveal(): Promise<void> {
  if (revealing.value) {
    return;
  }

  revealing.value = true;
  error.value = null;

  try {
    await revealInFinder(props.path);
  } catch (cause) {
    error.value = toUserMessage(cause, "无法在 Finder 中显示该文件。");
  } finally {
    revealing.value = false;
  }
}
</script>

<template>
  <span class="path-actions">
    <CopyButton :text="path" label="复制路径" />
    <button
      v-if="reveal"
      class="path-button"
      type="button"
      :disabled="revealing"
      @click="reveal"
    >
      {{ revealing ? "打开中…" : "Finder" }}
    </button>
    <span v-if="error" class="path-error" role="alert">{{ error }}</span>
  </span>
</template>

<style scoped>
.path-actions {
  display: inline-flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 6px;
}

.path-button {
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

.path-button:hover:not(:disabled) {
  border-color: var(--border-strong);
  background: var(--surface-hover);
  color: var(--text);
}

.path-button:disabled {
  cursor: default;
  opacity: 0.65;
}

.path-button:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 1px;
}

.path-error {
  color: var(--error);
  font-size: 10px;
}
</style>
