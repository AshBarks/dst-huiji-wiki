//! 制作栏图标页：集合小且稳定（过滤器 / 制作站，仅随游戏大更新变化）。
//! 一次全量渲染，按 images-sync 派生的语义分组（crafting.kind）分节，
//! 卡面以游戏内中英文名为主；常态应呈现「全部已同步」的摘要。
"use strict";

import { esc, getJSON, postJSON, $ } from "./util.js";
import { iconBadge, mountIconDetail } from "./icon_shared.js";

export async function pageCraftingIcons(main) {
  main.innerHTML = `
    <div class="panel">
      <div class="row" id="isum" style="flex-wrap:wrap">
        <span class="muted">加载中…</span>
      </div>
      <p class="muted" style="margin:8px 0 0">分组与名称由 images-sync 从游戏 Lua 定义 + 翻译表派生；点击图标查看历代版本并在弹窗内上传。新加入的图标以「新」标记。</p>
    </div>
    <div class="panel">
      <div id="igrid" class="icon-grid"></div>
    </div>`;

  const SECTIONS = [
    { kind: "filter", label: "制作栏过滤器" },
    { kind: "station", label: "制作站" },
    { kind: "other", label: "未分类" },
  ];

  const openDetail = mountIconDetail(main, render);
  let latestBuild = null;

  // 卡面主标题：游戏内中文名优先，其次英文名，最后退回文件名。
  const cardName = (it) => {
    const names = [it.name_zh, it.name_en].filter(Boolean);
    return names.length ? names.join(" / ") : it.file.replace(/\.png$/, "");
  };
  // 中文拼音序（浏览器 localeCompare），无中文名的退回文件名比较。
  const byName = (a, b) =>
    (cardName(a) || "").localeCompare(cardName(b) || "", "zh-Hans-CN", { numeric: true })
    || a.file.localeCompare(b.file);

  const statusLine = (counts, total) => {
    const bad = counts.missing + counts.unknown + counts.unuploadable + counts.unnamed;
    if (bad === 0) {
      return `<span class="badge b-success">全部已上传 ${total}</span>`;
    }
    const parts = [];
    if (counts.missing) parts.push(`未上传 ${counts.missing}`);
    if (counts.unknown) parts.push(`状态未知 ${counts.unknown}`);
    if (counts.unuploadable) parts.push(`无法上传 ${counts.unuploadable}`);
    if (counts.unnamed) parts.push(`无名称 ${counts.unnamed}`);
    return `<span class="badge b-err">${parts.join(" · ")}</span>`;
  };

  async function render() {
    const grid = $("#igrid");
    const sum = $("#isum");
    grid.innerHTML = "";
    let r;
    try {
      const p = new URLSearchParams({ sort: "name", source: "crafting", q: "", status: "all", page: 0, page_size: 500 });
      r = await getJSON(`/api/data/inventoryicons?${p}`);
    } catch (e) {
      sum.innerHTML = `<span style="color:var(--err)">加载失败：${esc(e.message)}</span>`;
      return;
    }
    latestBuild = r.latest_build ?? null;
    const items = [...r.items].sort(byName);

    const counts = { uploaded: 0, missing: 0, unknown: 0, unuploadable: 0, unnamed: 0 };
    for (const it of items) counts[it.status] = (counts[it.status] || 0) + 1;
    const isNew = (it) => latestBuild && it.first_build === latestBuild;

    sum.innerHTML = `
      <b>制作栏图标 ${r.total}</b>
      ${statusLine(counts, r.total)}
      ${r.meta_build ? `<span class="muted">元数据 build ${esc(r.meta_build)}</span>` : ""}
      ${counts.missing > 0
        ? `<button class="btn secondary mini" id="ibatch">批量上传缺失 ${counts.missing} 个</button>`
        : ""}`;

    const batchBtn = $("#ibatch");
    if (batchBtn) batchBtn.onclick = async () => {
      if (!confirm(`将向维基上传缺失的 ${counts.missing} 个制作栏图标。确定继续？`)) return;
      batchBtn.disabled = true;
      try {
        const j = await postJSON("/api/jobs", { kind: "upload_icons", source: "crafting", only_missing: true, wiki_dry_run: false });
        location.hash = `#/jobs/${j.id}`;
      } catch (err) { batchBtn.disabled = false; alert("提交失败：" + err.message); }
    };

    // 按语义分组分节（crafting.kind 由 images-sync 从游戏 Lua 定义派生）。
    const sectionOf = (it) => it.crafting?.kind ?? "other";
    for (const sec of SECTIONS) {
      const members = items.filter((it) => sectionOf(it) === sec.kind);
      if (!members.length) continue;
      grid.insertAdjacentHTML("beforeend",
        `<div class="icon-divider">${esc(sec.label)}（${members.length}）</div>`);
      for (const it of members) {
        const badge = iconBadge(it);
        const newBadge = isNew(it) ? '<span class="badge b-warn">新</span>' : "";
        grid.insertAdjacentHTML("beforeend", `
          <figure class="icon-card icon-card--named" data-file="${esc(it.file)}">
            <img loading="lazy" width="64" height="64"
              src="/static/split/crafting_menu_icons/${encodeURIComponent(it.file)}" alt="${esc(it.file)}">
            <figcaption>
              <b>${esc(cardName(it))}</b>
              <code class="muted">${esc(it.file.replace(/\.png$/, ""))}</code>
              ${badge || newBadge ? `<span>${badge} ${newBadge}</span>` : ""}
            </figcaption>
          </figure>`);
      }
    }
  }

  $("#igrid").addEventListener("click", (e) => {
    const card = e.target.closest(".icon-card");
    if (!card) return;
    openDetail(card.dataset.file);
  });

  await render();
}
