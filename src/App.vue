<script setup lang="ts">
// DevScope —— macOS 开发者工具箱
//
// 当前是空壳状态。后续会在这里挂载：
//   Dashboard / Environment / Project Inspector / Log Analyzer
//
// 所有与 Rust 的交互统一写在 src/services/rust-api.ts，不要直接散落在组件里。
</script>

<template>
  <main class="shell">
    <h1>DevScope</h1>
    <p class="muted">macOS 开发者工具箱 · 初始骨架</p>
  </main>
</template>

<style>
:root {
  font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", "Helvetica Neue", sans-serif;
  font-size: 14px;
  color: #1d1d1f;
  background: #f5f5f7;
  -webkit-font-smoothing: antialiased;
}

* {
  box-sizing: border-box;
}

body {
  margin: 0;
}

.shell {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100vh;
  gap: 4px;
}

h1 {
  margin: 0;
  font-size: 28px;
  font-weight: 600;
  letter-spacing: -0.02em;
}

.muted {
  margin: 0;
  color: #86868b;
}
</style>
