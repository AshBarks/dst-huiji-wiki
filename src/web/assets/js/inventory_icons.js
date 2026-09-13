//! 物品栏图标页：数量大、随 build 频繁更新。
//! 工具栏为「按加入历史（默认）/按文件名」排序 + 五态过滤 + 搜索；
//! 按 build 分组展示，组条内提供该 build 缺失图标的批量上传。
"use strict";

import { esc, getJSON, postJSON, $ } from "./util.js";
import { STATUS_LABELS, fmtDate, iconBadge, mountIconDetail } from "./icon_shared.js";

export async function pageInventoryIcons(main) {
  main.innerHTML = `
    <div class="panel">
      <div class="row">
        <div class="tabs" style="margin:0" id="isortTabs">
          <button class="tab active" data-sort="history">按加入历史</button>
          <button class="tab" data-sort="name">按文件名</button>
        </div>
        <select id="istat" title="按状态过滤">
          <option value="all">全部状态</option>
          <option value="uploaded">已上传</option>
          <option value="missing">未上传</option>
          <option value="unknown">状态未知</option>
          <option value="unuploadable">无法上传</option>
          <option value="unnamed">无英文名</option>
        </select>
        <input id="iq" placeholder="搜索文件名 / 中文名 / 英文名…" style="width:250px">
        <span class="muted" id="icount"></span>
      </div>
      <p class="muted" style="margin:8px 0 0">游戏更新后新 build 分组在最前，可整组批量上传缺失图标；点击图标查看历代版本并在弹窗内上传。名称与上传状态由 images-sync 刷新。</p>
    </div>
    <div class="panel">
      <div id="igrid" class="icon-grid"></div>
      <div id="isentinel" class="muted" style="text-align:center;padding:8px">加载中…</div>
    </div>`;

  // 排序组键：历史 → 首次加入 build；文件名 → 首字母（非字母归 #）。
  const groupKey = (it) => st.sort === "name"
    ? (/[a-z]/.test(it.file[0]) ? it.file[0].toUpperCase() : "#")
    : it.first_build;

  const st = { sort: "history", q: "", status: "all", page: 0, size: 120, loading: false, done: false, lastGroup: null, builds: {} };

  // 过滤下拉显示各状态数量（叠加搜索；由服务端统计）。
  const refreshStatusOptions = (counts) => {
    const sel = $("#istat");
    if (!sel || !counts) return;
    const total = Object.values(counts).reduce((a, b) => a + (b || 0), 0);
    for (const opt of sel.options) {
      const n = opt.value === "all" ? total : (counts[opt.value] || 0);
      opt.textContent = `${STATUS_LABELS[opt.value] || "全部状态"} ${n}`;
    }
  };

  const loadMore = async () => {
    if (st.loading || st.done) return;
    st.loading = true;
    try {
      const p = new URLSearchParams({ sort: st.sort, source: "inventory", q: st.q, status: st.status, page: st.page, page_size: st.size });
      const r = await getJSON(`/api/data/inventoryicons?${p}`);
      $("#icount").textContent = `共 ${r.total} 个图标 · 数据源 build ${r.latest_build ?? "—"}${r.meta_build ? ` · 元数据 build ${r.meta_build}` : ""}`;
      refreshStatusOptions(r.status_counts);
      if (r.builds) st.builds = r.builds;
      const grid = $("#igrid");
      for (const it of r.items) {
        const gk = groupKey(it);
        if (gk !== st.lastGroup) {
          st.lastGroup = gk;
          if (st.sort === "name") {
            grid.insertAdjacentHTML("beforeend", `<div class="icon-divider">${esc(gk)}</div>`);
          } else {
            const b = st.builds[gk] || {};
            const canBatch = (b.missing || 0) > 0;
            grid.insertAdjacentHTML("beforeend", `
              <div class="icon-divider">
                <span>Build ${esc(it.first_build)}${it.first_synced_at ? ` · ${fmtDate(it.first_synced_at)}` : ""} · 有名称 ${b.named || 0}/${b.total || 0}${b.missing ? ` · 缺 ${b.missing}` : ""}</span>
                ${canBatch ? `<button class="btn secondary mini" data-batch-build="${esc(gk)}">批量上传缺失 ${b.missing} 个</button>` : ""}
              </div>`);
          }
        }
        const names = [it.name_zh, it.name_en].filter(Boolean).join(" / ");
        const badge = iconBadge(it);
        grid.insertAdjacentHTML("beforeend", `
          <figure class="icon-card" data-file="${esc(it.file)}">
            <img loading="lazy" width="64" height="64"
              src="/static/split/inventoryimages/${encodeURIComponent(it.file)}" alt="${esc(it.file)}">
            <figcaption>
              <code>${esc(it.file.replace(/\.png$/, ""))}</code>
              ${names ? `<span class="muted">${esc(names)}</span>` : ""}
              ${badge ? `<span>${badge}</span>` : ""}
            </figcaption>
          </figure>`);
      }
      st.page += 1;
      st.done = grid.querySelectorAll(".icon-card").length >= r.total;
      $("#isentinel").textContent = st.done ? "已全部加载" : "继续滚动加载…";
    } catch (e) {
      $("#isentinel").textContent = `加载失败：${e.message}`;
    } finally {
      st.loading = false;
    }
    maybeMore();
  };

  // 页尾不足一屏时继续取，直到填满或加载完
  const maybeMore = () => {
    if (st.done || st.loading) return;
    requestAnimationFrame(() => {
      const r = $("#isentinel").getBoundingClientRect();
      if (r.top < window.innerHeight + 300) loadMore();
    });
  };

  const reset = () => {
    st.page = 0; st.lastGroup = null; st.done = false;
    $("#igrid").innerHTML = "";
    $("#isentinel").textContent = "加载中…";
    loadMore();
  };

  main.querySelectorAll("#isortTabs .tab").forEach(b => b.onclick = () => {
    if (b.dataset.sort === st.sort) return;
    st.sort = b.dataset.sort;
    main.querySelectorAll("#isortTabs .tab").forEach(x => x.classList.toggle("active", x === b));
    reset();
  });

  $("#istat").onchange = (e) => { st.status = e.target.value; reset(); };

  let deb;
  $("#iq").oninput = (e) => {
    clearTimeout(deb);
    deb = setTimeout(() => { st.q = e.target.value.trim(); reset(); }, 250);
  };

  const io = new IntersectionObserver((es) => {
    if (es.some(e => e.isIntersecting)) loadMore();
  }, { rootMargin: "300px" });
  io.observe($("#isentinel"));

  const openDetail = mountIconDetail(main, reset);

  $("#igrid").addEventListener("click", async (e) => {
    const batchBtn = e.target.closest("[data-batch-build]");
    if (batchBtn) {
      const build = batchBtn.dataset.batchBuild;
      const b = st.builds[build] || {};
      if (!confirm(`将向维基上传 Build ${build} 组中缺失的 ${b.missing || 0} 个物品栏图标（同名标题会自动去重）。确定继续？`)) return;
      batchBtn.disabled = true;
      try {
        const j = await postJSON("/api/jobs", { kind: "upload_icons", build, source: "inventory", only_missing: true, wiki_dry_run: false });
        location.hash = `#/jobs/${j.id}`;
      } catch (err) { batchBtn.disabled = false; alert("提交失败：" + err.message); }
      return;
    }
    const card = e.target.closest(".icon-card");
    if (!card) return;
    openDetail(card.dataset.file);
  });
}
