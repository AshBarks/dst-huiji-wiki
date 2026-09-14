//! 技能树图标页：321 个、低频变动；一次全量渲染，按角色分节。
//! 名称来自节点内联 title 引用 + PO 翻译（images-sync 派生），标题固定为
//! 本地文件 stem 的 MediaWiki 归一化，因此详情弹窗可直接复用。
"use strict";

import { createGroupedIconsPage } from "./source_icons.js";
import { STATUS_LABELS } from "./icon_shared.js";

// 角色花名册：中文名优先用于分节标题与拼音排序，英文名兜底未知 key。
const CHARACTERS = {
  walter: ["沃尔特", "Walter"],
  wathgrithr: ["薇格弗德", "Wigfrid"],
  wendy: ["温蒂", "Wendy"],
  willow: ["薇洛", "Willow"],
  wilson: ["威尔逊", "Wilson"],
  winona: ["薇诺娜", "Winona"],
  wolfgang: ["沃尔夫冈", "Wolfgang"],
  woodie: ["伍迪", "Woodie"],
  wormwood: ["沃姆伍德", "Wormwood"],
  wortox: ["沃拓克斯", "Wortox"],
  wurt: ["沃特", "Wurt"],
  wx78: ["WX-78", "WX-78"],
  wanda: ["旺达", "Wanda"],
  warly: ["沃利", "Warly"],
  wickerbottom: ["薇克巴顿", "Wickerbottom"],
  waxwell: ["麦斯威尔", "Maxwell"],
  wes: ["韦斯", "Wes"],
};

// 与物品栏页面状态过滤器保持一致的五态顺序。
const STATUS_ORDER = ["uploaded", "missing", "unknown", "unuploadable", "unnamed"];

const sectionOf = (it) => it.group?.kind === "skilltree" ? it.group.character : "other";

const sectionLabel = (key) => {
  const names = CHARACTERS[key];
  if (names) return names[0];
  return key === "other" ? "其他" : key.toUpperCase();
};

export const pageSkilltreeIcons = createGroupedIconsPage({
  source: "skilltree",
  title: "技能树图标",
  imageDir: "skilltree_icons",
  helpHtml: "按角色分节；可用上方过滤器按角色与图标状态筛选。节点内联 title 引用 + PO 翻译提供游戏内名称，生效标题 = 本地文件 stem 的维基归一化。点击图标查看历代版本并在弹窗内上传；孤儿图标仅展示与报告。",
  batchLabel: "批量上传缺失",
  batchConfirm: (n) => `将向维基上传缺失的 ${n} 个技能树图标。确定继续？`,
  sectionOf,
  sections: (items) => {
    const keys = [...new Set(items.map(sectionOf))];
    return keys
      .map((key) => ({ key, label: sectionLabel(key) }))
      .sort((a, b) =>
        a.label.localeCompare(b.label, "zh-Hans-CN", { numeric: true })
        || a.key.localeCompare(b.key));
  },
  filterDefs: (_items, sections) => [
    {
      id: "role",
      label: "角色",
      allLabel: "全部角色",
      valueOf: sectionOf,
      options: sections.map(({ key, label }) => ({ value: key, label })),
    },
    {
      id: "status",
      label: "图标状态",
      allLabel: "全部状态",
      valueOf: (it) => it.status,
      options: STATUS_ORDER.map((value) => ({ value, label: STATUS_LABELS[value] || value })),
    },
  ],
});
