<script setup lang="ts">
/**
 * 应用外壳：左侧导航 + 右侧路由出口。
 *
 * 「总览」展示本机配置和常用环境；「环境检测」是可折叠分组，子项按检测对象
 * 划分。项目分析、日志分析等尚未实现，先以不可点击的占位项展示。
 */
import { KeepAlive, ref, watch } from "vue";
import { RouterLink, RouterView, useRoute } from "vue-router";

const route = useRoute();
const envOpen = ref(true);

const upcoming = [
  { monogram: "项", label: "项目分析", hint: "Flutter 项目配置与依赖" },
  { monogram: "日", label: "日志分析", hint: "构建日志与错误统计" },
  { monogram: "报", label: "诊断报告", hint: "JSON / Markdown 导出" },
];

// 从总览或外部跳到环境页时，自动展开分组，避免当前页面入口被折叠隐藏。
watch(
  () => route.path,
  (path) => {
    if (path.startsWith("/environment/")) {
      envOpen.value = true;
    }
  },
  { immediate: true },
);
</script>

<template>
  <div class="app-shell">
    <aside class="sidebar">
      <div class="brand">
        <span class="brand-mark" aria-hidden="true">DS</span>
        <div class="brand-copy">
          <strong>DevScope</strong>
          <span>macOS 开发者工具箱</span>
        </div>
      </div>

      <nav class="nav" aria-label="主导航">
        <RouterLink class="nav-link" to="/" active-class="nav-link--active">
          <span class="nav-icon" aria-hidden="true">总</span>
          总览
        </RouterLink>

        <div class="nav-group">
          <button
            class="nav-group-toggle"
            type="button"
            :aria-expanded="envOpen"
            @click="envOpen = !envOpen"
          >
            <span class="nav-group-title">
              <span class="nav-icon" aria-hidden="true">环</span>
              环境检测
            </span>
            <span class="chevron" :class="{ 'chevron--open': envOpen }" aria-hidden="true">
              ⌄
            </span>
          </button>

          <div v-show="envOpen" class="nav-children">
            <RouterLink
              class="nav-link nav-link--child"
              to="/environment/flutter"
              active-class="nav-link--active"
            >
              Flutter
            </RouterLink>
            <RouterLink
              class="nav-link nav-link--child"
              to="/environment/android"
              active-class="nav-link--active"
            >
              Android
            </RouterLink>
            <RouterLink
              class="nav-link nav-link--child"
              to="/environment/ios"
              active-class="nav-link--active"
            >
              iOS / Xcode
            </RouterLink>
          </div>
        </div>

        <p class="nav-label">即将推出</p>
        <div v-for="item in upcoming" :key="item.label" class="nav-placeholder">
          <span class="nav-icon" aria-hidden="true">{{ item.monogram }}</span>
          <span class="placeholder-copy">
            <span>{{ item.label }}</span>
            <small>{{ item.hint }}</small>
          </span>
          <span class="soon">规划中</span>
        </div>
      </nav>

      <div class="sidebar-footer">
        <span class="local-dot" aria-hidden="true"></span>
        <span>本地检测 · 数据不上传</span>
      </div>
    </aside>

    <main class="content">
      <RouterView v-slot="{ Component }">
        <KeepAlive>
          <component :is="Component" />
        </KeepAlive>
      </RouterView>
    </main>
  </div>
</template>

<style scoped>
.app-shell {
  display: grid;
  grid-template-columns: 224px minmax(0, 1fr);
  min-height: 100vh;
}

.sidebar {
  position: sticky;
  top: 0;
  display: flex;
  flex-direction: column;
  gap: 24px;
  height: 100vh;
  padding: 22px 16px 18px;
  border-right: 1px solid rgba(0, 0, 0, 0.06);
  background: rgba(255, 255, 255, 0.58);
  backdrop-filter: saturate(180%) blur(20px);
  -webkit-backdrop-filter: saturate(180%) blur(20px);
}

.brand {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 0 8px;
}

.brand-mark {
  display: grid;
  place-items: center;
  width: 30px;
  height: 30px;
  border-radius: 9px;
  background: linear-gradient(145deg, #0071e3, #42a5f5);
  color: #ffffff;
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.02em;
  box-shadow: 0 2px 6px rgba(0, 113, 227, 0.24);
}

.brand-copy {
  display: flex;
  flex-direction: column;
  line-height: 1.2;
}

.brand-copy strong {
  font-size: 13px;
  font-weight: 650;
  letter-spacing: -0.01em;
}

.brand-copy span {
  color: var(--text-tertiary);
  font-size: 11px;
}

.nav {
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.nav-label {
  margin: 16px 10px 6px;
  color: var(--text-tertiary);
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 0.12em;
  text-transform: uppercase;
}

.nav-link {
  display: flex;
  align-items: center;
  gap: 9px;
  padding: 7px 10px;
  border-radius: 8px;
  color: var(--text-secondary);
  font-size: 12.5px;
  font-weight: 600;
  transition:
    background-color 0.15s ease,
    color 0.15s ease;
}

.nav-link:hover {
  background: rgba(0, 0, 0, 0.04);
  color: var(--text);
  text-decoration: none;
}

.nav-link--active {
  background: var(--surface);
  color: var(--text);
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.05);
}

.nav-icon {
  display: grid;
  place-items: center;
  flex-shrink: 0;
  width: 20px;
  height: 20px;
  border: 1px solid var(--border);
  border-radius: 6px;
  background: var(--surface-soft);
  color: var(--text-tertiary);
  font-size: 9px;
  font-weight: 700;
  line-height: 1;
}

.nav-link--active .nav-icon {
  border-color: transparent;
  background: var(--accent);
  color: #ffffff;
}

.nav-group {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.nav-group-toggle {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  width: 100%;
  padding: 7px 10px;
  border: 0;
  border-radius: 8px;
  background: transparent;
  color: var(--text-secondary);
  font-size: 12.5px;
  font-weight: 600;
  cursor: pointer;
}

.nav-group-toggle:hover {
  background: rgba(0, 0, 0, 0.04);
  color: var(--text);
}

.nav-group-toggle:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 1px;
}

.nav-group-title {
  display: flex;
  align-items: center;
  gap: 9px;
}

.chevron {
  color: var(--text-tertiary);
  font-size: 12px;
  line-height: 1;
  transition: transform 0.15s ease;
}

.chevron--open {
  transform: rotate(180deg);
}

.nav-children {
  display: flex;
  flex-direction: column;
  gap: 2px;
  margin-top: 2px;
}

.nav-link--child {
  padding-left: 39px;
  font-size: 12px;
  font-weight: 500;
}

.nav-placeholder {
  display: flex;
  align-items: center;
  gap: 9px;
  padding: 7px 10px;
  border-radius: 8px;
  color: var(--text-tertiary);
}

.nav-placeholder .nav-icon {
  opacity: 0.65;
}

.placeholder-copy {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
  line-height: 1.2;
}

.placeholder-copy span {
  font-size: 11.5px;
  font-weight: 600;
}

.placeholder-copy small {
  overflow: hidden;
  color: var(--text-tertiary);
  font-size: 9.5px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.soon {
  flex-shrink: 0;
  padding: 2px 5px;
  border: 1px solid var(--border);
  border-radius: 4px;
  background: var(--surface-soft);
  color: var(--text-tertiary);
  font-size: 9px;
  font-weight: 600;
}

.sidebar-footer {
  display: flex;
  align-items: center;
  gap: 7px;
  margin-top: auto;
  padding: 0 10px;
  color: var(--text-tertiary);
  font-size: 10.5px;
}

.local-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--ok);
}

.content {
  width: 100%;
  max-width: 1040px;
  min-width: 0;
  margin: 0 auto;
  padding: 34px 36px 56px;
}

@media (max-width: 820px) {
  .app-shell {
    grid-template-columns: 1fr;
  }

  .sidebar {
    position: static;
    height: auto;
    gap: 14px;
    padding: 14px 18px;
    border-right: 0;
    border-bottom: 1px solid rgba(0, 0, 0, 0.06);
  }

  .nav {
    flex-direction: row;
    flex-wrap: wrap;
    align-items: center;
  }

  .nav-label {
    width: 100%;
    margin: 8px 0 0;
  }

  .nav-group {
    flex-direction: row;
    align-items: center;
  }

  .nav-group-toggle {
    width: auto;
  }

  .nav-children {
    flex-direction: row;
    margin-top: 0;
  }

  .nav-link--child {
    padding-left: 10px;
  }

  .nav-placeholder {
    display: none;
  }

  .sidebar-footer {
    display: none;
  }

  .content {
    padding: 24px 20px 40px;
  }
}
</style>
