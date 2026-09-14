//! 制作栏图标页：集合小且稳定（过滤器 / 制作站，仅随游戏大更新变化）。
//! 复用按来源聚合页组件，按 images-sync 派生的语义分组（group.section）分节。
"use strict";

import { createGroupedIconsPage } from "./source_icons.js";

export const pageCraftingIcons = createGroupedIconsPage({
  source: "crafting",
  title: "制作栏图标",
  imageDir: "crafting_menu_icons",
  helpHtml: "分组与名称由 images-sync 从游戏 Lua 定义 + 翻译表派生；点击图标查看历代版本并在弹窗内上传。新加入的图标以「新」标记。",
  batchLabel: "批量上传缺失",
  batchConfirm: (n) => `将向维基上传缺失的 ${n} 个制作栏图标。确定继续？`,
  sectionOf: (it) => it.group?.kind === "crafting" ? it.group.section : "other",
  sections: () => [
    { key: "filter", label: "制作栏过滤器" },
    { key: "station", label: "制作站" },
    { key: "other", label: "未分类" },
  ],
});
