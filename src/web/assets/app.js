//! WebUI 入口：路由与启动（页面实现拆分在 ./js/ 下的 ES modules）。
"use strict";

import { $, esc, loadMeta, routeTimerRef } from "./js/util.js";
import { pageDashboard } from "./js/pages/dashboard.js";
import { pageRecipes } from "./js/pages/recipes.js";
import { pageTranslations } from "./js/pages/translations.js";
import { pageConstants } from "./js/pages/constants.js";
import { pageAnims } from "./js/pages/anims.js";
import { pageSnapshots } from "./js/pages/snapshots.js";
import { pageJobs, pageJobDetail } from "./js/jobs.js";
import { pageSkills } from "./js/skills.js";
import { pageInventoryIcons } from "./js/inventory_icons.js";
import { pageCraftingIcons } from "./js/crafting_icons.js";
import { pageSkilltreeIcons } from "./js/skilltree_icons.js";
import { pageAnimAssets } from "./js/anim_assets.js";

const routes = [
  ["", "概览", pageDashboard],
  ["jobs", "任务", pageJobs],
  ["recipes", "配方/材料", pageRecipes],
  ["translations", "翻译", pageTranslations],
  ["skills", "技能树", pageSkills],
  ["inventory-icons", "物品栏图标", pageInventoryIcons],
  ["crafting-icons", "制作栏图标", pageCraftingIcons],
  ["skilltree-icons", "技能树图标", pageSkilltreeIcons],
  ["constants", "常量", pageConstants],
  ["snapshots", "快照对比", pageSnapshots],
  ["anims", "动画对比", pageAnims],
  ["anim-assets", "动画素材", pageAnimAssets],
];

function navigate() { render(location.hash.replace(/^#\/?/, "") || ""); }


async function render(path) {
  if (routeTimerRef.id) { clearInterval(routeTimerRef.id); routeTimerRef.id = null; }
  const [head] = path.split("/");
  $("#nav").innerHTML = routes.map(([seg, label]) =>
    `<a href="#/${seg}" class="${seg === head ? "active" : ""}">${label}</a>`).join("");

  const main = $("#app");
  main.classList.toggle("wide", head === "anim-assets");
  main.innerHTML = `<p class="muted">加载中…</p>`;
  try {
    await loadMeta();
    const found = routes.find(([seg]) => seg === head) || routes[0];
    main.innerHTML = "";
    if (head === "jobs") {
      const [, id] = path.split("/");
      if (id) { await pageJobDetail(main, id); return; }
    }
    await found[2](main, path);
  } catch (e) {
    main.innerHTML = `<div class="panel" style="color:var(--err)">加载失败：${esc(e.message)}<pre style="white-space:pre-wrap;font-size:11px">${esc(e.stack || "")}</pre></div>`;
  }
}



/* ---------------- boot ---------------- */
window.addEventListener("hashchange", navigate);
// util 层的快照切换通过事件请求重渲染（避免循环导入）。
window.addEventListener("dstui:rerender", () =>
  render(location.hash.replace(/^#\/?/, "") || ""));
navigate();
