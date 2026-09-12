//! 物品图标页
"use strict";

import { esc, api, getJSON, postJSON, statusBadge, $ } from "./util.js";

export async function pageInventoryIcons(main) {
  main.innerHTML = `
    <div class="panel">
      <div class="row">
        <div class="tabs" style="margin:0" id="isrcTabs">
          <button class="tab active" data-source="all" data-label="全部来源">全部来源</button>
          <button class="tab" data-source="inventory" data-label="物品栏图标">物品栏图标</button>
          <button class="tab" data-source="crafting" data-label="制作栏图标">制作栏图标</button>
        </div>
        <div class="tabs" style="margin:0" id="isortTabs">
          <button class="tab active" data-sort="name">按文件名</button>
          <button class="tab" data-sort="history">按加入历史</button>
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
      <p class="muted" style="margin:8px 0 0">点击图标查看历代版本并在弹窗内上传；可手动填写 wiki 文件名并保存映射（默认 config/icon_title_overrides.json）。中文/英文名与上传状态由 images-sync 刷新。</p>
    </div>
    <div class="panel">
      <div id="igrid" class="icon-grid"></div>
      <div id="isentinel" class="muted" style="text-align:center;padding:8px">加载中…</div>
    </div>
    <div id="ivOverlay" class="iv-overlay" style="display:none">
      <div class="iv-panel">
        <div class="row" style="justify-content:space-between">
          <h2 id="ivTitle" style="margin:0"></h2>
          <button class="btn secondary" id="ivClose">关闭</button>
        </div>
        <div id="ivList"></div>
      </div>
    </div>`;

  const fmtDate = (ms) => ms ? new Date(ms).toLocaleString() : "";
  const groupKey = (it) => st.sort === "name"
    ? (/[a-z]/.test(it.file[0]) ? it.file[0].toUpperCase() : "#")
    : it.first_build;

  const STATUS_LABELS = {
    uploaded: "已上传", missing: "未上传", unknown: "状态未知",
    unuploadable: "无法上传", unnamed: "无英文名",
  };
  const iconBadge = (it) => {
    if (!it.status) return "";
    if (it.status === "uploaded") return '<span class="badge b-success">已上传</span>';
    if (it.status === "missing") return '<span class="badge b-err">未上传</span>';
    if (it.status === "unuploadable") return `<span class="badge b-warn" title="${esc(it.note || "")}">无法上传</span>`;
    if (it.status === "unknown") return '<span class="badge b-muted">状态未知</span>';
    return `<span class="badge b-muted">${esc(STATUS_LABELS[it.status] || it.status)}</span>`;
  };

  const st = { sort: "name", source: "all", q: "", status: "all", page: 0, size: 120, loading: false, done: false, lastGroup: null, builds: {} };

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

  // 来源标签显示各来源图标总数。
  const refreshSourceTabs = (sources) => {
    const tabs = $("#isrcTabs");
    if (!tabs || !sources) return;
    const total = sources.reduce((a, x) => a + (x.total || 0), 0);
    tabs.querySelectorAll(".tab").forEach((b) => {
      const n = b.dataset.source === "all"
        ? total
        : (sources.find((x) => x.id === b.dataset.source)?.total || 0);
      b.textContent = `${b.dataset.label} ${n}`;
    });
  };

  const loadMore = async () => {
    if (st.loading || st.done) return;
    st.loading = true;
    try {
      const p = new URLSearchParams({ sort: st.sort, source: st.source, q: st.q, status: st.status, page: st.page, page_size: st.size });
      const r = await getJSON(`/api/data/inventoryicons?${p}`);
      $("#icount").textContent = `共 ${r.total} 个图标 · 数据源 build ${r.latest_build ?? "—"}${r.meta_build ? ` · 元数据 build ${r.meta_build}` : ""}`;
      refreshStatusOptions(r.status_counts);
      refreshSourceTabs(r.sources);
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
        const srcDir = it.source === "crafting" ? "crafting_menu_icons" : "inventoryimages";
        grid.insertAdjacentHTML("beforeend", `
          <figure class="icon-card" data-file="${esc(it.file)}">
            <img loading="lazy" width="64" height="64"
              src="/static/split/${srcDir}/${encodeURIComponent(it.file)}" alt="${esc(it.file)}">
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

  main.querySelectorAll("#isrcTabs .tab").forEach(b => b.onclick = () => {
    if (b.dataset.source === st.source) return;
    st.source = b.dataset.source;
    main.querySelectorAll("#isrcTabs .tab").forEach(x => x.classList.toggle("active", x === b));
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

  // 历代版本 + 上传/映射编辑抽屉
  const overlay = $("#ivOverlay");
  $("#ivClose").onclick = () => { overlay.style.display = "none"; };
  overlay.onclick = (e) => { if (e.target === overlay) overlay.style.display = "none"; };

  const openDetail = async (file) => {
    $("#ivTitle").textContent = file;
    $("#ivList").innerHTML = '<p class="muted">加载中…</p>';
    overlay.style.display = "flex";
    try {
      const r = await getJSON(`/api/data/inventoryicons/versions?file=${encodeURIComponent(file)}`);
      const names = [r.name_zh, r.name_en].filter(Boolean).join(" / ");
      const exists = !!(r.wiki && r.wiki.exists === true);
      const statusBadge = iconBadge(r);
      const source = r.title_source === "override" ? "映射" : r.title_source === "auto" ? "自动" : "无";
      const target = r.wiki_title
        ? `<div class="muted">当前生效：<code>${esc(r.wiki_title)}</code>（${source}）</div>`
        : "";
      const link = r.wiki && r.wiki.url
        ? `<div class="muted" style="word-break:break-all">当前文件：<a href="${esc(r.wiki.url)}" target="_blank" rel="noopener">${esc(r.wiki.url)}</a></div>`
        : "";
      const editor = `
        <div class="iv-editor">
          <input id="ivName" value="${esc(r.title || "")}" placeholder="维基文件名，如 Pick-Axe.png">
          <div class="row" style="gap:6px;flex-wrap:wrap">
            <button class="btn" id="ivUpload"${r.present ? "" : " disabled"}>${exists ? "重新上传（覆盖）" : "上传到维基"}</button>
            <button class="btn secondary" id="ivSaveMap">保存映射</button>
            ${r.title_source === "override" ? '<button class="btn secondary" id="ivClearMap">恢复自动命名</button>' : ""}
          </div>
          ${r.present ? "" : '<div class="muted">该图标不在当前 build 产物中，无法上传。</div>'}
        </div>`;
      const head = `
        <div class="iv-actions">
          <div>${names ? `<b>${esc(names)}</b>` : '<span class="muted">未找到 STRINGS.NAMES 名称</span>'} ${statusBadge}</div>
          ${target}
          ${link}
          ${editor}
        </div>`;
      $("#ivList").innerHTML = head + (r.versions.map((v, i) => `
        <div class="icon-ver">
          <img loading="lazy" width="64" height="64" src="/static/objects/${esc(v.hash)}" alt="">
          <div>
            <b>Build ${esc(v.build)}</b>${v.synced_at ? `<span class="muted"> · ${fmtDate(v.synced_at)}</span>` : ""}
            ${i === 0 && r.present ? ' <span class="badge b-success">当前</span>' : ""}
          </div>
          <code class="muted" style="margin-left:auto">${esc(v.hash.slice(0, 12))}…</code>
        </div>`).join("") || '<p class="muted">无历史记录。</p>');

      const readTitle = () => ($("#ivName").value || "").trim();
      const ub = $("#ivUpload");
      if (ub) ub.onclick = async () => {
        const title = readTitle();
        if (!title) { alert("请先填写维基文件名"); return; }
        const msg = `将上传 ${file} 到 File:${title}${exists ? "（同名已存在则覆盖重传）" : ""}，并保存到映射表。确定继续？`;
        if (!confirm(msg)) return;
        ub.disabled = true;
        try {
          const j = await postJSON("/api/jobs", {
            kind: "upload_icons", file, title, wiki_dry_run: false,
          });
          location.hash = `#/jobs/${j.id}`;
        } catch (err) { ub.disabled = false; alert("提交失败：" + err.message); }
      };
      const saveBtn = $("#ivSaveMap");
      if (saveBtn) saveBtn.onclick = async () => {
        const title = readTitle();
        if (!title) { alert("请先填写维基文件名"); return; }
        saveBtn.disabled = true;
        try {
          await postJSON("/api/data/inventoryicons/title", { file, title });
          reset();
          await openDetail(file);
        } catch (err) { saveBtn.disabled = false; alert("保存失败：" + err.message); }
      };
      const clearBtn = $("#ivClearMap");
      if (clearBtn) clearBtn.onclick = async () => {
        if (!confirm(`清除 ${file} 的映射并恢复自动命名？`)) return;
        clearBtn.disabled = true;
        try {
          await postJSON("/api/data/inventoryicons/title", { file, title: null });
          reset();
          await openDetail(file);
        } catch (err) { clearBtn.disabled = false; alert("清除失败：" + err.message); }
      };
    } catch (err) {
      $("#ivList").innerHTML = `<p style="color:var(--err)">加载失败：${esc(err.message)}</p>`;
    }
  };

  $("#igrid").addEventListener("click", async (e) => {
    const batchBtn = e.target.closest("[data-batch-build]");
    if (batchBtn) {
      const build = batchBtn.dataset.batchBuild;
      const b = st.builds[build] || {};
      if (!confirm(`将向维基上传 Build ${build} 组中缺失的 ${b.missing || 0} 个图标（同名标题会自动去重）。确定继续？`)) return;
      batchBtn.disabled = true;
      try {
        const payload = { kind: "upload_icons", build, only_missing: true, wiki_dry_run: false };
        if (st.source !== "all") payload.source = st.source;
        const j = await postJSON("/api/jobs", payload);
        location.hash = `#/jobs/${j.id}`;
      } catch (err) { batchBtn.disabled = false; alert("提交失败：" + err.message); }
      return;
    }
    const card = e.target.closest(".icon-card");
    if (!card) return;
    openDetail(card.dataset.file);
  });
}

/* ---------------- constants ---------------- */
