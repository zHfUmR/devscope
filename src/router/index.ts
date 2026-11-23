/**
 * 应用路由。
 *
 * Tauri 的 WebView 用自定义协议加载页面，没有后端做 history fallback；
 * 因此使用 hash 路由，刷新后不会因为 `/environment/flutter` 找不到文件而白屏。
 */

import { createRouter, createWebHashHistory } from "vue-router";
import AndroidEnvironment from "../views/AndroidEnvironment.vue";
import FlutterEnvironment from "../views/FlutterEnvironment.vue";
import IosEnvironment from "../views/IosEnvironment.vue";
import Overview from "../views/Overview.vue";

const router = createRouter({
  history: createWebHashHistory(),
  // 页面切换时回到顶部，避免从很长的详情页进入总览时停在中间。
  scrollBehavior: () => ({ top: 0 }),
  routes: [
    {
      path: "/",
      name: "overview",
      component: Overview,
      meta: { title: "总览" },
    },
    {
      path: "/environment/flutter",
      name: "flutter",
      component: FlutterEnvironment,
      meta: { title: "Flutter" },
    },
    {
      path: "/environment/android",
      name: "android",
      component: AndroidEnvironment,
      meta: { title: "Android" },
    },
    {
      path: "/environment/ios",
      name: "ios",
      component: IosEnvironment,
      meta: { title: "iOS / Xcode" },
    },
    {
      path: "/:pathMatch(.*)*",
      redirect: "/",
    },
  ],
});

router.afterEach((to) => {
  const title = typeof to.meta.title === "string" ? to.meta.title : null;
  document.title = title ? `${title} · DevScope` : "DevScope";
});

export default router;
