//! 按来源聚合的图标仪表盘页（制作栏 / 技能树图标共用）。
//!
//! 集合小且低频变化，因此一次全量渲染、无搜索无分页；具体分节规则、图片
//! 目录与文案由调用方传入（见 crafting_icons.js / skilltree_icons.js），
//! 详情弹窗与映射编辑复用 icon_shared.js。
"use strict";

import { esc, getJSON, postJSON, $ } from "./util.js";
import { iconBadge, mountIconDetail } from "./icon_shared.js";

/// 生成一个图标来源页组件；`cfg` 字段：
/// - `source` / `imageDir`：API 来源标识与 `/static/split/<dir>/` 图片目录；
/// - `title` / `helpHtml` / `batchLabel` / `batchConfirm`；
/// - `sectionOf(it)`：条目 → 分节 key；
/// - `sections(items)`：条目 → 有序的 `{ key, label }` 列表；
/// - `filterDefs(items, sections)`：可选，返回过滤器定义数组，每项为
///   `{ id, label, allLabel, valueOf(it), options: [{ value, label }] }`；
///   多个过滤器按 AND 组合，选项计数按“其余过滤器生效后”的集合统计。
export function createGroupedIconsPage(cfg) {
  return async function page(main) {
    const hasFilters = typeof cfg.filterDefs === "function";
    main.innerHTML = `
      <div class="panel">
        ${hasFilters ? '<div class="row" id="ifilters" style="margin-bottom:10px"></div>' : ""}
        <div class="row" id="isum" style="flex-wrap:wrap">
          <span class="muted">加载中…</span>
        </div>
        <p class="muted" style="margin:8px 0 0">${cfg.helpHtml}</p>
      </div>
      <div class="panel">
        <div id="igrid" class="icon-grid"></div>
      </div>`;

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
      if (bad === 0) return `<span class="badge b-success">全部已上传 ${total}</span>`;
      const parts = [];
      if (counts.missing) parts.push(`未上传 ${counts.missing}`);
      if (counts.unknown) parts.push(`状态未知 ${counts.unknown}`);
      if (counts.unuploadable) parts.push(`无法上传 ${counts.unuploadable}`);
      if (counts.unnamed) parts.push(`无名称 ${counts.unnamed}`);
      return `<span class="badge b-err">${parts.join(" · ")}</span>`;
    };

    const openDetail = mountIconDetail(main, render);
    let latestBuild = null;
    const filterState = {};

    /// 当前筛选值是否命中；`exceptId` 用于计算某过滤器各选项的交叉计数。
    const matchesFilters = (it, defs, exceptId) => defs.every((def) => {
      if (exceptId && def.id === exceptId) return true;
      const value = filterState[def.id] ?? "all";
      return value === "all" || String(def.valueOf(it)) === String(value);
    });

    const paintFilters = (defs, items) => {
      const wrap = $("#ifilters");
      if (!wrap) return;
      wrap.innerHTML = defs.map((def) => {
        // 选项计数排除自身，这样两个维度可以互相反映交叉分布。
        const counts = new Map();
        let total = 0;
        for (const it of items) {
          if (!matchesFilters(it, defs, def.id)) continue;
          total += 1;
          const value = String(def.valueOf(it));
          counts.set(value, (counts.get(value) || 0) + 1);
        }
        const current = filterState[def.id] ?? "all";
        const options = def.options.map((opt) => {
          const value = String(opt.value);
          return `<option value="${esc(value)}"${current === value ? " selected" : ""}>`
            + `${esc(opt.label)}（${counts.get(value) || 0}）</option>`;
        }).join("");
        return `<label style="display:flex;align-items:center;gap:6px;margin:0;color:var(--muted)">
          <span>${esc(def.label || "过滤")}</span>
          <select title="${esc(def.label || "过滤")}">
            <option value="all"${current === "all" ? " selected" : ""}>${esc(def.allLabel || "全部")}（${total}）</option>
            ${options}
          </select>
        </label>`;
      }).join("");

      wrap.querySelectorAll("select").forEach((sel, i) => {
        const def = defs[i];
        sel.onchange = () => {
          filterState[def.id] = sel.value;
          render();
        };
      });
    };

    async function render() {
      const grid = $("#igrid");
      const sum = $("#isum");
      grid.innerHTML = "";
      let r;
      try {
        const p = new URLSearchParams({
          sort: "name", source: cfg.source, q: "", status: "all",
          page: 0, page_size: 500,
        });
        r = await getJSON(`/api/data/inventoryicons?${p}`);
      } catch (e) {
        sum.innerHTML = `<span style="color:var(--err)">加载失败：${esc(e.message)}</span>`;
        return;
      }
      latestBuild = r.latest_build ?? null;
      const items = [...r.items].sort(byName);
      const allSections = cfg.sections(items);
      const defs = hasFilters ? cfg.filterDefs(items, allSections) : [];
      for (const def of defs) {
        if (!(def.id in filterState)) filterState[def.id] = "all";
      }
      const filteredItems = hasFilters ? items.filter((it) => matchesFilters(it, defs)) : items;
      if (hasFilters) paintFilters(defs, items);

      const counts = { uploaded: 0, missing: 0, unknown: 0, unuploadable: 0, unnamed: 0 };
      for (const it of items) counts[it.status] = (counts[it.status] || 0) + 1;
      const isNew = (it) => latestBuild && it.first_build === latestBuild;

      sum.innerHTML = `
        <b>${esc(cfg.title)} ${r.total}</b>
        ${statusLine(counts, r.total)}
        ${r.redirect_placeholders
          ? `<span class="badge b-warn" title="站内已有指向旧文件的 File 重定向占位页；仅报告，不自动覆盖">重定向占位 ${r.redirect_placeholders}（仅报告）</span>`
          : ""}
        ${r.meta_build ? `<span class="muted">元数据 build ${esc(r.meta_build)}</span>` : ""}
        ${hasFilters ? `<span class="badge b-muted">筛选后 ${filteredItems.length} / ${items.length}</span>` : ""}
        ${counts.missing > 0
          ? `<button class="btn secondary mini" id="ibatch">${esc(cfg.batchLabel)} ${counts.missing} 个</button>`
          : ""}`;

      const batchBtn = $("#ibatch");
      if (batchBtn) batchBtn.onclick = async () => {
        if (!confirm(cfg.batchConfirm(counts.missing))) return;
        batchBtn.disabled = true;
        try {
          const j = await postJSON("/api/jobs", {
            kind: "upload_icons", source: cfg.source, only_missing: true, wiki_dry_run: false,
          });
          location.hash = `#/jobs/${j.id}`;
        } catch (err) {
          batchBtn.disabled = false;
          alert("提交失败：" + err.message);
        }
      };

      const sections = cfg.sections(filteredItems);
      if (!sections.length) {
        grid.insertAdjacentHTML("beforeend", `
          <p class="muted">${hasFilters ? "当前筛选条件下暂无图标。" : "暂无可展示的图标。"}</p>`);
        return;
      }
      for (const sec of sections) {
        const members = filteredItems.filter((it) => cfg.sectionOf(it) === sec.key);
        if (!members.length) continue;
        grid.insertAdjacentHTML("beforeend",
          `<div class="icon-divider">${esc(sec.label)}（${members.length}）</div>`);
        for (const it of members) {
          const badge = iconBadge(it);
          const newBadge = isNew(it) ? '<span class="badge b-warn">新</span>' : "";
          grid.insertAdjacentHTML("beforeend", `
            <figure class="icon-card icon-card--named" data-file="${esc(it.file)}">
              <img loading="lazy" width="64" height="64"
                src="/static/split/${cfg.imageDir}/${encodeURIComponent(it.file)}" alt="${esc(it.file)}">
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
  };
}
