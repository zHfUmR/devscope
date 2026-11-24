<script setup lang="ts">
/**
 * Cmd+K 命令面板。
 *
 * 只做页面级快速跳转：内置所有路由，外加一个「最近项目」入口（复用项目页
 * 已经持久化的路径）。不做搜索业务数据，避免把系统读取搬到前端。
 */
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { useRouter } from "vue-router";

interface CommandAction {
  id: string;
  label: string;
  hint: string;
  group: string;
  to: string;
}

const props = defineProps<{
  open: boolean;
}>();

const emit = defineEmits<{
  (event: "update:open", value: boolean): void;
}>();

const router = useRouter();
const query = ref("");
const activeIndex = ref(0);
const input = ref<HTMLInputElement | null>(null);
const list = ref<HTMLUListElement | null>(null);
const recentProject = ref<string | null>(null);

const STORAGE_KEY = "devscope.project.path";

const actions = computed<CommandAction[]>(() => {
  const navigation: CommandAction[] = [
    { id: "overview", label: "总览", hint: "本机配置与常用环境", group: "页面", to: "/" },
    {
      id: "flutter",
      label: "Flutter 环境",
      hint: "SDK、版本与 flutter doctor",
      group: "环境检测",
      to: "/environment/flutter",
    },
    {
      id: "android",
      label: "Android 环境",
      hint: "SDK / JDK / AVD",
      group: "环境检测",
      to: "/environment/android",
    },
    {
      id: "ios",
      label: "iOS / Xcode 环境",
      hint: "Xcode / CocoaPods / 模拟器",
      group: "环境检测",
      to: "/environment/ios",
    },
    {
      id: "toolchains",
      label: "常用工具链",
      hint: "Python / Node.js / Git / Rust / Go / Java",
      group: "环境检测",
      to: "/environment/toolchains",
    },
    {
      id: "project",
      label: "项目分析",
      hint: "pubspec / 依赖 / 平台配置",
      group: "项目与日志",
      to: "/project",
    },
    {
      id: "log",
      label: "日志分析",
      hint: "构建日志中的错误与警告",
      group: "项目与日志",
      to: "/logs",
    },
    {
      id: "report",
      label: "诊断报告",
      hint: "预览并导出 JSON / Markdown",
      group: "项目与日志",
      to: "/report",
    },
  ];

  if (!recentProject.value) {
    return navigation;
  }

  return [
    {
      id: "recent-project",
      label: "打开最近项目",
      hint: recentProject.value,
      group: "最近",
      to: "/project",
    },
    ...navigation,
  ];
});

const filtered = computed<CommandAction[]>(() => {
  const keyword = query.value.trim().toLowerCase();
  if (!keyword) {
    return actions.value;
  }

  return actions.value.filter((action) =>
    `${action.label} ${action.hint} ${action.group}`.toLowerCase().includes(keyword),
  );
});

function readRecentProject(): string | null {
  try {
    return localStorage.getItem(STORAGE_KEY);
  } catch {
    return null;
  }
}

function close(): void {
  emit("update:open", false);
}

function choose(action: CommandAction): void {
  close();
  void router.push(action.to);
}

function move(delta: number): void {
  const total = filtered.value.length;
  if (total === 0) {
    return;
  }

  activeIndex.value = (activeIndex.value + delta + total) % total;
  void nextTick(() => {
    const item = list.value?.querySelector<HTMLElement>(
      `[data-index="${activeIndex.value}"]`,
    );
    item?.scrollIntoView({ block: "nearest" });
  });
}

function onKeydown(event: KeyboardEvent): void {
  if (!props.open) {
    return;
  }

  if (event.key === "Escape") {
    event.preventDefault();
    close();
    return;
  }
  if (event.key === "ArrowDown") {
    event.preventDefault();
    move(1);
    return;
  }
  if (event.key === "ArrowUp") {
    event.preventDefault();
    move(-1);
    return;
  }
  if (event.key === "Enter") {
    event.preventDefault();
    const action = filtered.value[activeIndex.value];
    if (action) {
      choose(action);
    }
  }
}

watch(
  () => props.open,
  async (open) => {
    if (!open) {
      return;
    }

    query.value = "";
    activeIndex.value = 0;
    recentProject.value = readRecentProject();
    await nextTick();
    input.value?.focus();
  },
);

watch(query, () => {
  activeIndex.value = 0;
});

onMounted(() => {
  window.addEventListener("keydown", onKeydown);
});

onUnmounted(() => {
  window.removeEventListener("keydown", onKeydown);
});
</script>

<template>
  <Teleport to="body">
    <Transition name="palette">
      <div v-if="open" class="palette-overlay" @click.self="close">
        <div
          class="palette"
          role="dialog"
          aria-modal="true"
          aria-label="快速跳转"
        >
          <input
            ref="input"
            v-model="query"
            class="palette-input"
            type="text"
            placeholder="搜索页面…"
            autocomplete="off"
            spellcheck="false"
            @keydown.stop="onKeydown"
          />

          <ul v-if="filtered.length > 0" ref="list" class="palette-list" role="listbox">
            <li
              v-for="(action, index) in filtered"
              :key="action.id"
              :data-index="index"
              class="palette-item"
              :class="{ 'palette-item--active': index === activeIndex }"
              role="option"
              :aria-selected="index === activeIndex"
              @mouseenter="activeIndex = index"
              @click="choose(action)"
            >
              <span class="palette-label">{{ action.label }}</span>
              <span class="palette-hint">{{ action.hint }}</span>
              <span class="palette-group">{{ action.group }}</span>
            </li>
          </ul>

          <div v-else class="palette-empty">没有匹配的页面</div>

          <div class="palette-footer">
            <span><kbd>↑</kbd><kbd>↓</kbd> 选择</span>
            <span><kbd>↵</kbd> 打开</span>
            <span><kbd>esc</kbd> 关闭</span>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.palette-overlay {
  position: fixed;
  inset: 0;
  z-index: 100;
  display: flex;
  align-items: flex-start;
  justify-content: center;
  padding: 14vh 16px 16px;
  background: rgba(0, 0, 0, 0.28);
  backdrop-filter: blur(8px);
  -webkit-backdrop-filter: blur(8px);
}

.palette {
  width: min(560px, 100%);
  overflow: hidden;
  border: 1px solid var(--border);
  border-radius: 14px;
  background: var(--surface);
  box-shadow: var(--shadow);
}

.palette-input {
  width: 100%;
  padding: 15px 16px;
  border: 0;
  border-bottom: 1px solid var(--border);
  outline: none;
  background: transparent;
  color: var(--text);
  font-size: 14px;
}

.palette-input::placeholder {
  color: var(--text-tertiary);
}

.palette-list {
  max-height: 360px;
  margin: 0;
  padding: 6px;
  overflow: auto;
  list-style: none;
}

.palette-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 9px 10px;
  border-radius: 8px;
  cursor: pointer;
}

.palette-item--active {
  background: var(--surface-hover);
}

.palette-label {
  flex-shrink: 0;
  color: var(--text);
  font-size: 12.5px;
  font-weight: 600;
}

.palette-hint {
  min-width: 0;
  flex: 1;
  overflow: hidden;
  color: var(--text-tertiary);
  font-size: 11px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.palette-group {
  flex-shrink: 0;
  padding: 2px 6px;
  border: 1px solid var(--border);
  border-radius: 5px;
  color: var(--text-tertiary);
  font-size: 9.5px;
}

.palette-empty {
  padding: 28px 16px;
  color: var(--text-secondary);
  font-size: 12.5px;
  text-align: center;
}

.palette-footer {
  display: flex;
  gap: 14px;
  padding: 8px 14px;
  border-top: 1px solid var(--border);
  color: var(--text-tertiary);
  font-size: 10.5px;
}

kbd {
  display: inline-block;
  min-width: 16px;
  margin-right: 3px;
  padding: 1px 4px;
  border: 1px solid var(--border);
  border-radius: 4px;
  background: var(--surface-soft);
  color: var(--text-secondary);
  font-family: inherit;
  font-size: 9.5px;
  text-align: center;
}

.palette-enter-active,
.palette-leave-active {
  transition: opacity 0.12s ease;
}

.palette-enter-active .palette,
.palette-leave-active .palette {
  transition: transform 0.12s ease;
}

.palette-enter-from,
.palette-leave-to {
  opacity: 0;
}

.palette-enter-from .palette,
.palette-leave-to .palette {
  transform: translateY(-6px) scale(0.99);
}

@media (prefers-reduced-motion: reduce) {
  .palette-enter-active,
  .palette-leave-active,
  .palette-enter-active .palette,
  .palette-leave-active .palette {
    transition: none;
  }
}
</style>
