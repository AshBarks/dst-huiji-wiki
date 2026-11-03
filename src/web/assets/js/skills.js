//! 技能树页：渲染唯一实现是 零件:Skilltree.js（`src/service/assets/skilltree_widget.js`）。
//!
//! 本模块只做三件事：拉取 `/api/viz/skilltree`、拼出渲染器约定的
//! `.skilltree` DOM（`data-character` + 内嵌 `.def-json` + `.to-svg`）、
//! 注入本地素材路径。此前这里是一份独立的渲染实现，与维基版本靠人工同步。
"use strict";

import { esc, getJSON, META } from "./util.js";

const WIDGET_SRC = "/static/js/skilltree_widget.js";

let widgetPromise = null;

/// 按需加载渲染器（只加载一次）。
function loadWidget() {
  if (window.SkilltreeWidget) return Promise.resolve(window.SkilltreeWidget);
  if (!widgetPromise) {
    widgetPromise = new Promise((resolve, reject) => {
      const script = document.createElement("script");
      script.src = WIDGET_SRC;
      script.onload = () => resolve(window.SkilltreeWidget);
      script.onerror = () => reject(new Error(`加载 ${WIDGET_SRC} 失败`));
      document.head.appendChild(script);
    });
  }
  return widgetPromise;
}

// 渲染器在维基上走 Special:FilePath；预览里换成本地 images-sync 的 split 目录。
window.skilltreeAssetUrl = (kind, file) =>
  `/static/split/${kind}/${encodeURIComponent(file)}.png`;

/// 把 `/api/viz/skilltree` 的 `{nodes}` 转成渲染器消费的模块数据 `{defs, metainfo}`。
///
/// 渲染器沿用维基模块的形状：坐标是 `pos: [x, y]`（接口给的是 `x`/`y`），
/// 其余字段（connects/locks/lock_open/icon/title/desc…）逐一对应。
function toModuleData(tree) {
  const defs = {};
  for (const node of tree.nodes || []) {
    defs[node.name] = { ...node, pos: [node.x, node.y] };
  }
  return { defs, metainfo: tree.metainfo || {} };
}

export async function pageSkills(main) {
  const chars = META.skill_characters || [];
  main.innerHTML = `
    <div class="panel">
      <div class="row">
        <label style="margin:0">角色</label>
        <select id="charSel">${chars.map((c) => `<option>${esc(c)}</option>`).join("")}</select>
        <span class="muted">渲染与交互由 零件:Skilltree.js 唯一实现（点击聚焦、空格学习、左下/右下按钮）。</span>
      </div>
    </div>
    <div class="panel"><div id="skwrap"></div></div>`;

  const sel = main.querySelector("#charSel");
  const wrap = main.querySelector("#skwrap");
  if (chars.includes("wilson")) sel.value = "wilson";
  else if (chars.length) sel.value = chars[0];

  async function draw() {
    wrap.textContent = "加载中…";
    const tree = await getJSON(`/api/viz/skilltree?character=${encodeURIComponent(sel.value)}`);
    const widget = await loadWidget();
    // 结构必须与 模块:Skilltree 页内的片段一致（见 output/skilltree_wiki_pipeline.wikitext）：
    // 渲染器读取 `.skilltree` 的 data-character、`.def-json` 的 innerText，
    // 并把 `.to-svg` 替换成 SVG；标题/描述/富文本容器由宿主提供。
    wrap.innerHTML = `<div class="skilltree" data-character="${esc(sel.value)}">
      <div class="def-json" style="display:none"></div>
      <div style="overflow-x:auto"><div class="to-svg"></div></div>
      <div class="skilltree-skill-title"></div>
      <div class="skilltree-skill-desc"></div>
      <div class="skilltree-skill-description-wrapper"></div>
    </div>`;
    wrap.querySelector(".def-json").textContent = JSON.stringify(toModuleData(tree));
    widget.buildAll();
  }

  sel.onchange = () => {
    draw().catch((e) => { wrap.textContent = `渲染失败：${e.message}`; });
  };
  await draw();
}
