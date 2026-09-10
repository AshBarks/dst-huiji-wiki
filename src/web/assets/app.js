/* DST 灰机维基维护台 - WebUI (vanilla JS, no dependencies) */
"use strict";

const $ = (s) => document.querySelector(s);
const esc = (s) => String(s ?? "").replace(/[&<>"']/g,
  c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));

let META = null;            // /api/data/meta for currentSnapshot
let currentSnapshot = "";   // "" = latest
let routeTimer = null;

async function api(path, opts) {
  const res = await fetch(path, opts);
  if (!res.ok) {
    let detail = res.statusText;
    try { const j = await res.json(); if (j.error) detail = j.error; } catch {}
    throw new Error(`${res.status}: ${detail}`);
  }
  return res.json();
}
const getJSON = (p) => api(p);
const postJSON = (p, body) => api(p, {
  method: "POST", headers: { "Content-Type": "application/json" },
  body: JSON.stringify(body || {})
});

function fmtTime(ms) {
  if (!ms) return "—";
  return new Date(ms).toLocaleTimeString();
}

/* ---------------- router ---------------- */
const routes = [
  ["", "概览", pageDashboard],
  ["jobs", "任务", pageJobs],
  ["recipes", "配方/材料", pageRecipes],
  ["translations", "翻译", pageTranslations],
  ["skills", "技能树", pageSkills],
  ["icons", "物品图标", pageInventoryIcons],
  ["constants", "常量", pageConstants],
  ["snapshots", "快照对比", pageSnapshots],
  ["anims", "动画对比", pageAnims],
  ["anim-assets", "动画素材", pageAnimAssets],
];

function navigate() { render(location.hash.replace(/^#\/?/, "") || ""); }

async function render(path) {
  if (routeTimer) { clearInterval(routeTimer); routeTimer = null; }
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
    main.innerHTML = `<div class="panel" style="color:var(--err)">加载失败：${esc(e.message)}</div>`;
  }
}

async function loadMeta() {
  META = await getJSON(`/api/data/meta?snapshot=${encodeURIComponent(currentSnapshot)}`);
}

/* ---------------- shared widgets ---------------- */
function statusBadge(status) {
  return `<span class="badge b-${esc(status)}">${esc(status)}</span>`;
}

function elFromHtml(html) {
  const t = document.createElement("template");
  t.innerHTML = html.trim();
  return t.content.firstElementChild;
}

function diffCardHtml(d) {
  const lines = (d.text || "").split("\n").map(l => {
    let cls = "";
    if (l.startsWith("+++") || l.startsWith("---")) cls = "d-head";
    else if (l.startsWith("@@")) cls = "d-hunk";
    else if (l.startsWith("+")) cls = "d-add";
    else if (l.startsWith("-")) cls = "d-del";
    return `<div class="${cls}">${esc(l)}</div>`;
  }).join("");
  return `
    <div class="diff-card">
      <div class="diff-head">
        <code>${esc(d.page)}</code>
        <span class="badge b-ok">+${d.added}</span>
        <span class="badge b-err">-${d.removed}</span>
        <span class="spacer"></span>
        <button class="btn secondary mini" data-copy>复制</button>
      </div>
      <pre class="diff-body">${lines}</pre>
    </div>`;
}

function snapshotSelect(onChangeId) {
  return `<select id="${onChangeId}" title="数据版本">
    <option value="">最新</option>
  </select>`;
}
async function fillSnapshots(selectEl) {
  const r = await getJSON("/api/snapshots");
  for (const s of r.snapshots) {
    const opt = document.createElement("option");
    opt.value = s.name; opt.textContent = s.name;
    selectEl.appendChild(opt);
  }
  selectEl.value = currentSnapshot;
  selectEl.addEventListener("change", async () => {
    currentSnapshot = selectEl.value;
    await render(location.hash.replace(/^#\/?/, "") || "");
  });
}

function pager(total, page, pageSize, onGo) {
  const pages = Math.max(1, Math.ceil(total / pageSize));
  const wrap = document.createElement("div");
  wrap.className = "pager";
  wrap.innerHTML = `
    <button class="btn secondary" ${page <= 0 ? "disabled" : ""}>上一页</button>
    <span class="muted">第 ${page + 1} / ${pages} 页 · 共 ${total} 条</span>
    <button class="btn secondary" ${(page + 1) >= pages ? "disabled" : ""}>下一页</button>`;
  const [prev, , next] = wrap.querySelectorAll("button, span, button");
  prev.onclick = () => onGo(page - 1);
  next.onclick = () => onGo(page + 1);
  return wrap;
}

/* ---------------- dashboard ---------------- */
async function pageDashboard(main) {
  let jobs = [];
  try { jobs = (await getJSON("/api/jobs")).jobs.slice(0, 6); } catch {}
  const snaps = (await getJSON("/api/snapshots")).snapshots.length;
  const cfg = await getJSON("/api/config");
  const ss = cfg.scripts_sync || {};
  const scriptsDisabled = !ss.ready || ss.up_to_date;
  const scriptsStatus = !ss.ready
    ? "无法读取 version.txt，请检查 DST__ROOT"
    : ss.up_to_date
      ? `已是最新版本：${esc(ss.detected_version)}`
      : `记录 ${esc(ss.recorded_version || "无")} → 检测 ${esc(ss.detected_version)}，可同步`;

  main.innerHTML = `
    <div class="card-row">
      <div class="card"><div class="num">${META.recipe_count}</div><span class="muted">配方</span></div>
      <div class="card"><div class="num">${META.ingredient_count}</div><span class="muted">材料种类</span></div>
      <div class="card"><div class="num">${META.po_total_entries}</div><span class="muted">翻译条目</span></div>
      <div class="card"><div class="num">${META.tuning_count}</div><span class="muted">TUNING 常量</span></div>
      <div class="card"><div class="num">${snaps}</div><span class="muted">历史快照</span></div>
    </div>
    <div class="panel">
      <h2>快捷操作</h2>
      <div class="row">
        <button class="btn" id="quickScriptsSync" ${scriptsDisabled ? "disabled" : ""}>scripts-sync 同步游戏脚本</button>
        <span class="muted">${scriptsStatus}</span>
      </div>
    </div>
    <div class="panel">
      <h2>最近任务</h2>
      <table><thead><tr><th>状态</th><th>类型</th><th>创建</th><th>结束</th></tr></thead>
      <tbody>${jobs.map(j => `
        <tr style="cursor:pointer" onclick="location.hash='#/jobs/${j.id}'">
          <td>${statusBadge(j.status)}</td><td><code>${esc(j.kind)}</code></td>
          <td>${fmtTime(j.created_at_ms)}</td><td>${fmtTime(j.finished_at_ms)}</td>
        </tr>`).join("") || '<tr><td colspan="4" class="muted">暂无任务</td></tr>'}
      </tbody></table>
    </div>
    <div class="panel muted">
      数据来源：<b>${esc(META.label)}</b>${currentSnapshot ? `（快照 ${esc(currentSnapshot)}）` : ""}
    </div>`;

  const quickSyncBtn = $("#quickScriptsSync");
  if (quickSyncBtn) quickSyncBtn.onclick = async () => {
    if (!confirm("将执行 scripts-sync 同步游戏脚本（真实本地操作）。确定继续？")) return;
    const j = await postJSON("/api/jobs", { kind: "scripts_sync" });
    location.hash = `#/jobs/${j.id}`;
  };
}

/* ---------------- jobs ---------------- */
// Each entry: label; wiki: true => the job edits wiki pages, so the global
// "wiki dry-run" checkbox applies and dry_run=false needs an extra confirm.
// Local-only jobs (wiki: false) gate their own disk writes via JobKind
// fields of the same shape; the global checkbox is hidden for them.
const JOB_DEFS = {
  maintain_item_table: { label: "维护物品表 → 维基", wiki: true },
  maintain_dst_recipes: { label: "维护配方表 → 维基", wiki: true },
  maintain_copy_clip: {
    label: "维护模块常量 → 维基", wiki: true,
    fields: [{ k: "type", label: "类型：rbtl / tech / filters / names（留空=全部）" }],
  },
  skill_tree_wiki: {
    label: "维护技能树子页面 → 维基", wiki: true,
    fields: [
      { k: "character", label: "角色过滤子串（留空=全部，如 walter）" },
      { k: "output", label: "产物目录（可选，写 <Char>.lua 文件）" },
    ],
  },
  skill_tree_export: {
    label: "技能树数据导出到本地",
    fields: [
      { k: "character", label: "角色过滤子串（留空=全部，如 walter）" },
      { k: "output", label: "输出目录（可选，默认 output/skilltree）" },
    ],
  },
  prefab_overrides: {
    label: "预制体重定向解析",
    fields: [{ k: "input", label: "Lua 文件路径" }],
  },
  scripts_sync: {
    label: "scripts-sync 同步游戏脚本",
    fields: [
      { k: "force", type: "check", label: "强制重新同步（版本相同也执行）" },
      { k: "dry_run", type: "check", label: "演练：只报告计划、不动任何文件" },
      { k: "state_path", label: "版本状态文件（可选，默认 ./dst_version.txt）", default: "./dst_version.txt" },
    ],
  },
  images_sync: {
    label: "images-sync 处理游戏图片",
    fields: [
      { k: "force", type: "check", label: "忽略增量与幂等检查，全量重跑" },
      { k: "dry_run", type: "check", label: "演练：只盘点并报告计划、不写文件" },
    ],
  },
  anim_sync: {
    label: "anim-sync 动画历史同步",
    fields: [
      { k: "force", type: "check", label: "强制重新归档（忽略幂等检查）" },
      { k: "dry_run", type: "check", label: "演练：只盘点并报告计划、不写文件" },
      { k: "label", label: "版本标签（可选，默认 version.txt）" },
      { k: "out", label: "历史根目录（可选，默认 $ANIM__OUT_DIR 或 output/anim）" },
    ],
  },
  anim_diff: {
    label: "anim-diff 动画目录对比",
    fields: [
      { k: "old", label: "旧动画目录" },
      { k: "new", label: "新动画目录" },
      { k: "zip", label: "只对比某个相对路径（可选，如 dynamic/abigail_ice.dyn）" },
    ],
  },
  skin_index: {
    label: "skin-index 生成皮肤动画索引",
    fields: [
      { k: "scripts", label: "游戏脚本根目录（留空=DST__ROOT/data/databundles/scripts）" },
      { k: "anim", label: "动画资源目录（可选，默认由 scripts 推导）" },
      { k: "out", label: "输出 JSON（可选，默认 output/skin-index.json）" },
    ],
  },
};

async function pageJobs(main) {
  main.innerHTML = `
    <div class="panel">
      <h2>提交新任务</h2>
      <div class="row">
        <select id="jobKind">${Object.entries(JOB_DEFS)
          .map(([k, v]) => `<option value="${k}">${v.label}</option>`).join("")}</select>
        <label id="dryRunRow" style="margin:0;display:flex;align-items:center;gap:6px;color:var(--text)">
          <input type="checkbox" id="dryRun" checked> 维基干跑（不写入维基，仅出 diff）</label>
        <button class="btn" id="submitJob">提交任务</button>
      </div>
      <div id="jobFields"></div>
      <p class="muted" id="jobHint"></p>
    </div>
    <div class="panel">
      <h2>任务列表</h2>
      <div id="jobList"></div>
    </div>`;

  const kindSel = $("#jobKind");
  const hasDryRunField = (def) => (def.fields || []).some(f => (Array.isArray(f) ? f[0] : f.k) === "dry_run");
  const renderFields = () => {
    const def = JOB_DEFS[kindSel.value];
    $("#dryRunRow").style.display = def.wiki ? "" : "none";
    $("#jobFields").innerHTML = (def.fields || [])
      .map(f => {
        const [k, label] = Array.isArray(f) ? f : [f.k, f.label];
        const defaultValue = !Array.isArray(f) && typeof f.default === "function"
          ? f.default()
          : (!Array.isArray(f) ? (f.default || "") : "");
        if (!Array.isArray(f) && f.type === "check")
          return `<label style="display:flex;align-items:center;gap:6px;margin:6px 0;color:var(--text)">
            <input type="checkbox" data-field="${k}" ${defaultValue ? "checked" : ""}> ${esc(label)}</label>`;
        return `<label>${esc(label)}<input style="width:100%" data-field="${k}" value="${esc(defaultValue)}"></label>`;
      }).join("");
    const hasDryRun = hasDryRunField(def);
    $("#jobHint").textContent = def.wiki
      ? "涉及维基写入的任务在维基干跑下只生成 diff 预览；取消勾选并经确认后直接写入。"
      : hasDryRun
        ? "纯本地任务不写维基；其参数中的“演练”勾选决定是否真实改动本地文件。"
        : "纯本地任务不写维基，直接在本地执行。";
  };
  kindSel.onchange = renderFields;
  renderFields();

  $("#submitJob").onclick = async () => {
    const def = JOB_DEFS[kindSel.value];
    const body = { kind: kindSel.value };
    if (def.wiki) body.wiki_dry_run = $("#dryRun").checked;
    document.querySelectorAll("#jobFields input[data-field]").forEach(i => {
      if (i.type === "checkbox") { if (i.checked) body[i.dataset.field] = true; }
      else { const v = i.value.trim(); if (v !== "") body[i.dataset.field] = v; }
    });
    if (def.wiki) {
      if (!body.wiki_dry_run && !confirm("已关闭维基干跑：任务可能直接修改维基页面。确定继续？")) return;
    } else if (hasDryRunField(def) && !body.dry_run) {
      // Local task has an explicit rehearsal switch and it is off => real writes.
      if (!confirm(`${def.label} 将真实执行本地文件操作（非演练）。确定继续？`)) return;
    } else if (!hasDryRunField(def)) {
      // Local task without a rehearsal switch always performs real local I/O.
      if (!confirm(`${def.label} 将真实执行本地文件操作。确定继续？`)) return;
    }
    try {
      const j = await postJSON("/api/jobs", body);
      location.hash = `#/jobs/${j.id}`;
    } catch (e) { alert("提交失败：" + e.message); }
  };

  const refresh = async () => {
    const jobs = (await getJSON("/api/jobs")).jobs;
    $("#jobList").innerHTML = `<table><thead><tr><th>状态</th><th>类型</th><th>创建</th><th>耗时</th><th></th></tr></thead>
      <tbody>${jobs.map(j => {
        const dur = j.finished_at_ms && j.started_at_ms
          ? ((j.finished_at_ms - j.started_at_ms) / 1000).toFixed(1) + "s"
          : (j.status === "running" ? "进行中…" : "—");
        return `<tr style="cursor:pointer" onclick="location.hash='#/jobs/${j.id}'">
          <td>${statusBadge(j.status)}</td><td>${j.touches_wiki && j.wiki_dry_run ? '<span class="badge b-warn">维基干跑</span> ' : ""}<code>${esc(j.kind)}</code></td>
          <td>${fmtTime(j.created_at_ms)}</td><td>${dur}</td>
          <td class="muted">${j.event_count} 条日志</td></tr>`;
      }).join("") || '<tr><td colspan="5" class="muted">暂无任务</td></tr>'}</tbody></table>`;
  };
  await refresh();
  routeTimer = setInterval(refresh, 3000);
}

async function pageJobDetail(main, id) {
  main.innerHTML = `
    <div class="panel" id="jdHead"></div>
    <div class="panel">
      <div class="tabs">
        <button class="tab active" data-tab="log">日志</button>
        <button class="tab" data-tab="diff" id="tabDiff">Diff 预览</button>
      </div>
      <div class="log" id="jdLog"></div>
      <div id="jdDiffs" style="display:none"></div>
    </div>`;
  const logEl = $("#jdLog");
  const diffEl = $("#jdDiffs");
  const tabDiffBtn = $("#tabDiff");
  const tabBtns = main.querySelectorAll(".tab");
  let doneSeen = false;
  let diffCount = 0;

  tabBtns.forEach(b => b.onclick = () => {
    tabBtns.forEach(x => x.classList.toggle("active", x === b));
    const showDiff = b.dataset.tab === "diff";
    logEl.style.display = showDiff ? "none" : "";
    diffEl.style.display = showDiff ? "" : "none";
  });

  const appendDiff = (d) => {
    diffCount++;
    tabDiffBtn.textContent = `Diff 预览 (${diffCount})`;
    diffEl.appendChild(elFromHtml(diffCardHtml(d)));
    const copyBtn = diffEl.lastElementChild.querySelector("[data-copy]");
    if (copyBtn) copyBtn.onclick = () => navigator.clipboard.writeText(d.text);
  };

  const appendEv = (ev) => {
    if (ev.type === "log") logEl.textContent += ev.text + "\n";
    else if (ev.type === "stage") logEl.textContent += `\n========== ${ev.name} ==========\n`;
    else if (ev.type === "diff") appendDiff(ev);
    else if (ev.type === "done") doneSeen = true;
    logEl.scrollTop = logEl.scrollHeight;
  };

  const renderHead = (h) => {
    const canApprove = h.touches_wiki && h.wiki_dry_run && h.status === "success";
    $("#jdHead").innerHTML = `
    <div class="row">
      <h2 style="margin:0"><code>${esc(h.kind)}</code> ${statusBadge(h.status)}</h2>
      ${canApprove ? `<button class="btn" id="approveBtn">批准并真实写入</button>` : ""}
      <button class="btn danger" id="cancelBtn">取消任务</button>
      <button class="btn secondary" onclick="location.hash='#/jobs'">返回列表</button>
    </div>
    ${h.error ? `<p style="color:var(--err)">错误：${esc(h.error)}</p>` : ""}
    ${h.result && h.result.status === "rollback_skipped"
      ? `<p style="color:var(--warn)">检测到版本回退（${esc(h.result.build || h.result.label)} < 已记录 ${esc(h.result.recorded_build || h.result.recorded_label)}）：本次未执行任何操作，未写入任何文件。</p>`
      : ""}
    <details><summary class="muted">任务参数</summary><pre>${esc(JSON.stringify(h.params, null, 2))}</pre></details>
    ${h.result ? `<details open><summary>执行结果</summary><pre>${esc(JSON.stringify(h.result, null, 2))}</pre></details>` : ""}`;
    const cb = $("#cancelBtn");
    if (cb) cb.onclick = async () => { await postJSON(`/api/jobs/${id}/cancel`); };
    if (cb) cb.disabled = !["queued", "running"].includes(h.status);
    const ab = $("#approveBtn");
    if (ab) ab.onclick = async () => {
      if (!confirm("将使用相同参数真实写入维基页面（不再走维基干跑）。确定继续？")) return;
      ab.disabled = true;
      try {
        const j = await postJSON("/api/jobs", Object.assign({}, h.params, { wiki_dry_run: false }));
        location.hash = `#/jobs/${j.id}`;
      } catch (e) { ab.disabled = false; alert("提交失败：" + e.message); }
    };
  };

  const head = await getJSON(`/api/jobs/${id}`);
  renderHead(head);
  for (const ev of head.logs || []) appendEv(ev);
  for (const d of head.diffs || []) appendDiff(d);
  const terminal = ["success", "failed", "cancelled"].includes(head.status);
  if (terminal) {
    doneSeen = true;
  }

  if (!doneSeen && !terminal) {
    // Ask the SSE stream to skip the events we already rendered from the
    // REST detail response. This prevents duplicated logs/diffs after a
    // refresh while still replaying any events that arrived in between.
    const seenCount = (head.logs || []).length;
    const es = new EventSource(`/api/jobs/${id}/events?since=${seenCount}`);
    es.onmessage = (msg) => {
      try { appendEv(JSON.parse(msg.data)); } catch {}
      if (doneSeen) { es.close(); refreshHead(); }
    };
    es.onerror = () => { if (doneSeen) es.close(); };
  } else { doneSeen = true; }

  async function refreshHead() { renderHead(await getJSON(`/api/jobs/${id}`)); }
}

/* ---------------- recipes & ingredients ---------------- */
async function pageRecipes(main) {
  main.innerHTML = `
    <div class="panel">
      <div class="row">
        <input id="rq" placeholder="搜索配方名…" style="width:220px">
        <select id="rtech"><option value="">全部科技</option></select>
        <input id="ring" placeholder="材料反查（点击表格中的材料也可）…" style="width:260px">
        <button class="btn secondary" id="rclear">清空筛选</button>
        <span class="muted" id="rcount"></span>
      </div>
    </div>
    <div class="panel"><div id="rtable"></div><div id="rpager"></div></div>`;

  for (const t of META.tech_levels) {
    const o = document.createElement("option"); o.textContent = t; $("#rtech").appendChild(o);
  }

  const state = { q: "", tech: "", ing: "", page: 0, size: 60 };
  const load = async () => {
    const p = new URLSearchParams({ q: state.q, tech: state.tech, ingredient: state.ing,
      page: state.page, page_size: state.size });
    const r = await getJSON(`/api/data/recipes?${p}`);
    $("#rcount").textContent = `共 ${r.total} 个配方`;
    $("#rtable").innerHTML = `<table><thead><tr><th>名称</th><th>科技</th><th>材料</th></tr></thead><tbody>
      ${r.items.map(x => `<tr>
        <td><code>${esc(x.name)}</code></td><td>${esc(x.tech)}</td>
        <td>${x.ingredients.map(i =>
          `<span class="chip" data-ing="${esc(i.item)}">${esc(i.item)} ×${i.amount}</span>`).join("")}
        </td></tr>`).join("")}</tbody></table>`;
    $("#rpager").innerHTML = "";
    $("#rpager").appendChild(pager(r.total, r.page, r.page_size, (pg) => { state.page = pg; load(); }));
    document.querySelectorAll("#rtable .chip").forEach(c =>
      c.onclick = () => { state.ing = c.dataset.ing; $("#ring").value = state.ing; state.page = 0; load(); });
  };

  let deb;
  $("#rq").oninput = (e) => { clearTimeout(deb); deb = setTimeout(() => { state.q = e.target.value.trim(); state.page = 0; load(); }, 250); };
  $("#ring").oninput = (e) => { clearTimeout(deb); deb = setTimeout(() => { state.ing = e.target.value.trim(); state.page = 0; load(); }, 250); };
  $("#rtech").onchange = (e) => { state.tech = e.target.value; state.page = 0; load(); };
  $("#rclear").onclick = () => { state.q = state.tech = state.ing = ""; $("#rq").value = $("#ring").value = ""; $("#rtech").value = ""; state.page = 0; load(); };
  await load();
}

/* ---------------- translations ---------------- */
async function pageTranslations(main) {
  main.innerHTML = `
    <div class="panel"><h2>分类进度（点击查看条目）</h2><div id="tchart"></div></div>
    <div class="panel">
      <h2 id="ttitle">全部条目</h2>
      <div class="row">
        <input id="tq" placeholder="搜索…" style="width:220px">
        <select id="ttf"><option value="">全部</option><option value="yes">已翻译</option><option value="no">未翻译</option></select>
      </div>
      <div id="ttable" style="margin-top:10px"></div><div id="tpager"></div>
    </div>`;

  const cats = META.po_categories.filter(c => c.category !== "NONE").slice(0, 18);
  const maxTotal = Math.max(...cats.map(c => c.total), 1);
  const W = 1060, rowH = 26, barW = W - 320;
  const svgParts = cats.map((c, i) => {
    const y = i * rowH + 14;
    const w1 = (c.total / maxTotal) * barW;
    const w2 = (c.translated / maxTotal) * barW;
    const pct = c.total ? Math.round(100 * c.translated / c.total) : 0;
    return `<g class="bar-wrap" data-cat="${esc(c.category)}" style="cursor:pointer">
      <text x="0" y="${y + 4}" fill="var(--muted)" font-size="12">${esc(c.category)}</text>
      <rect x="150" y="${y - 8}" width="${w1}" height="16" rx="4" fill="#2c3547"/>
      <rect x="150" y="${y - 8}" width="${w2}" height="16" rx="4" fill="#3fb96e"/>
      <text x="${150 + w1 + 8}" y="${y + 4}" fill="var(--muted)" font-size="11">${pct}% (${c.translated}/${c.total})</text>
    </g>`;
  }).join("");
  $("#tchart").innerHTML = `<svg viewBox="0 0 ${W} ${cats.length * rowH + 20}" width="100%">${svgParts}</svg>`;

  const st = { cat: null, q: "", tf: "", page: 0, size: 50 };
  const loadEntries = async () => {
    const p = new URLSearchParams({ category: st.cat || "", q: st.q, translated: st.tf,
      page: st.page, page_size: st.size });
    const r = await getJSON(`/api/data/po/entries?${p}`);
    $("#ttable").innerHTML = `<table><thead><tr><th>上下文 msgctxt</th><th>英文 msgid</th><th>中文 msgstr</th></tr></thead><tbody>
      ${r.items.map(e => `<tr>
        <td class="muted"><small>${esc(e.msgctxt || "")}</small></td>
        <td>${esc(e.msgid)}</td>
        <td style="${e.msgstr.trim() ? "" : "color:var(--err)"}">${esc(e.msgstr) || "（空）"}</td>
      </tr>`).join("")}</tbody></table>`;
    $("#tpager").innerHTML = "";
    $("#tpager").appendChild(pager(r.total, r.page, r.page_size, (pg) => { st.page = pg; loadEntries(); }));
  };

  document.querySelectorAll("#tchart g.bar-wrap").forEach(g => g.onclick = async () => {
    st.cat = g.dataset.cat; st.page = 0;
    $("#ttitle").textContent = `分类：${st.cat}`;
    await loadEntries();
    document.querySelector("#ttitle").scrollIntoView({ behavior: "smooth" });
  });
  let deb;
  $("#tq").oninput = (e) => { clearTimeout(deb); deb = setTimeout(() => { st.q = e.target.value.trim(); st.page = 0; loadEntries(); }, 250); };
  $("#ttf").onchange = (e) => { st.tf = e.target.value; st.page = 0; loadEntries(); };
  await loadEntries();
}

/* ---------------- skill trees ---------------- */
async function pageSkills(main) {
  const chars = META.skill_characters || [];
  main.innerHTML = `
    <div class="panel">
      <div class="row">
        <label style="margin:0">角色</label>
        <select id="charSel">${chars.map(c => `<option>${esc(c)}</option>`).join("")}</select>
        <span class="muted">布局与交互 1:1 还原游戏内界面（参考零件:Skilltree.js）。</span>
        <span class="muted" id="skillXp"></span>
      </div>
      <p class="muted" style="margin:6px 0 0">点击技能/锁查看详情；可选技能可通过“学习”按钮或空格键点亮；双击背景或“重置洞察”清空已学技能；lock 节点按学习情况与外部条件自动解锁。</p>
    </div>
    <div class="panel"><div id="skwrap"></div>
      <div class="skilltree-info" id="skInfo">
        <b id="skTitle"></b>
        <p id="skDesc" class="muted"></p>
      </div>
    </div>`;

  const sel = $("#charSel");
  sel.value = chars.includes("wilson") ? "wilson" : chars[0];

  // 常量与零件:Skilltree.js 完全一致。
  const WIDTH = 600;
  const HEIGHT = 540;
  const SVG_HEIGHT = 460;
  const X_SCALE = 1;
  const X_OFFSET = -2;          // 原始文件中背景图 bg_tree xoffset 2
  const Y_SCALE = 1.157;        // 原始文件高度 756 缩到 460 的补偿
  const Y_OFFSET = 50 + 30 - 20;// tree yoffset -50 + panel -30 + bg_tree -20
  const ICON_SIZE = 28;
  const ICON_BUTTON_SIZE = 32;
  const LOCK_SIZE = 28 * 0.8;
  const SKILL_FOCUS_SIZE = 40;
  const LOCK_FOCUS_SIZE = 40;
  const TOTAL_XP = 15;
  const XP_SIZE = 50;
  const X_XP = 1;
  const Y_XP = 215 - 50 + 20;
  const buttonWidth = 180;
  const buttonHeight = 43;
  const buttonLeftX = WIDTH / 2 - 40 - buttonWidth;
  const buttonRightX = WIDTH / 2 + 40;
  const buttonY = HEIGHT - 170;

  const skillAsset = (name) => `/static/split/skilltree/${encodeURIComponent(name)}.png`;
  const reduxAsset = (name) => `/static/split/global_redux/${encodeURIComponent(name)}.png`;
  const iconUrl = (icon) => `/static/split/skilltree_icons/${encodeURIComponent(icon)}.png`;

  let tree = null;
  let skills = {};   // name -> node（icon 存在的节点）
  let locks = {};    // name -> node（lock_open 节点）
  let parents = {};  // skill -> 直接父技能（connects 指向它的技能）

  let activatedSkills = new Set();
  let focusing = null;
  // 每个节点的渲染引用
  let gfx = {};      // name -> {bg, icon, isLock, infographic, x, y}
  let skillFocusEle = null;
  let lockFocusEle = null;
  let learnBtn = null;       // {normal, hover, down, learned, text}
  let resetBtn = null;

  function yScale() {
    return ["wendy", "wortox"].includes(sel.value.toLowerCase()) ? 1 : Y_SCALE;
  }
  const px = (x) => WIDTH / 2 + X_SCALE * (X_OFFSET + x);
  const py = (y) => HEIGHT / 2 - yScale() * (y - Y_OFFSET);

  function buildMaps(nodes) {
    skills = {}; locks = {}; parents = {};
    for (const n of nodes) {
      if (n.icon) skills[n.name] = n;
      else if (n.lock_open !== undefined) locks[n.name] = n;
    }
    for (const n of nodes) {
      if (skills[n.name]) parents[n.name] = [];
    }
    for (const n of nodes) for (const c of (n.connects || [])) {
      if (skills[n.name] && parents[c]) parents[c].push(n.name);
      // 与零件:Skilltree.js 相同：锁的 connects 子技能把该锁并入自己的
      // locks（"只有一个lock的skill会没有locks"），进入 must_have_all_of。
      if (locks[n.name] && skills[c]) {
        const child = skills[c];
        child.locks = child.locks || [];
        if (!child.locks.includes(n.name)) child.locks.push(n.name);
      }
    }
  }

  const remainingXp = () => TOTAL_XP - activatedSkills.size;

  function countTags(tag) {
    let count = 0;
    for (const name of activatedSkills) {
      const skill = skills[name];
      if (skill && skill.tags && skill.tags.includes(tag)) count += 1;
    }
    return count;
  }

  // lock_open 声明式条件求值（与零件:Skilltree.js 的 checkLockOpen 一致）。
  function evalLockCond(cond) {
    if (cond === true || cond === false || typeof cond === "number" || typeof cond === "string") return cond;
    if (typeof cond !== "object" || cond === null) return false;
    if (cond.Achievement) return true; // 外部成就类条件在本地默认视为已解锁。
    for (const key in cond) {
      const val = cond[key];
      switch (key) {
        case "GreaterThan": return evalLockCond(val.left) > evalLockCond(val.right);
        case "GreaterOrEqThan": return evalLockCond(val.left) >= evalLockCond(val.right);
        case "LessThan": return evalLockCond(val.left) < evalLockCond(val.right);
        case "LessOrEqThan": return evalLockCond(val.left) <= evalLockCond(val.right);
        case "Eq": return evalLockCond(val.left) === evalLockCond(val.right);
        case "And": return evalLockCond(val.left) && evalLockCond(val.right);
        case "Or": return evalLockCond(val.left) || evalLockCond(val.right);
        case "Not": return !evalLockCond(val);
        case "CountTags": return countTags(val);
        case "CountSkills": return activatedSkills.size;
        case "ActivatedSkill": return activatedSkills.has(val);
        case "Add": return evalLockCond(val.left) + evalLockCond(val.right);
        default: return false;
      }
    }
    return false;
  }

  function isLockOpen(name) {
    const lock = locks[name];
    // 未显式给出条件时默认视为已解锁：外部成就等条件本地无法验证。
    if (!lock || lock.lock_open === undefined || lock.lock_open === null) return true;
    return !!evalLockCond(lock.lock_open);
  }

  // 可学条件与零件:Skilltree.js / 游戏激活校验（ValidateCharacterData 的
  // must_have_one_of / must_have_all_of）一致：
  //   root，或（所有 locks 打开 且 有父技能被激活——无父技能视为可达）。
  // 注意：skilltreebuilder 的显示循环虽然对带 locks 的技能只检查锁，
  // 但真正学习要过服务端校验，父技能激活（或父为已开锁）仍是前提。
  function statusOf(name) {
    const skill = skills[name];
    if (!skill) return null;
    if (skill.infographic) return "selected";
    if (activatedSkills.has(name)) return "selected";
    if (remainingXp() <= 0) return "unselected";
    const unlocked = !(skill.locks || []).some(l => !isLockOpen(l));
    const reachable = !(parents[name] || []).length
      || parents[name].some(p => activatedSkills.has(p));
    return (skill.root || (unlocked && reachable)) ? "selectable" : "unselected";
  }

  function canLearn(name) {
    return skills[name] && !skills[name].infographic && statusOf(name) === "selectable";
  }

  function setBg(g, name, hover) {
    const href = skillAsset(hover ? name + "_over" : name);
    if (g._href !== href) { g.bg.setAttribute("href", href); g._href = href; }
  }

  function svgImg(href, x, y, w, h, extra) {
    return `<image href="${esc(href)}" x="${x}" y="${y}" width="${w}" height="${h}" ${extra || ""}preserveAspectRatio="xMidYMid meet"/>`;
  }

  function infoTitle(name) {
    const n = tree.nodes.find(m => m.name === name);
    if (!n) return name;
    if (n.icon) return n.title || n.name;
    return isLockOpen(name) ? "已解锁路径" : "路径锁定";
  }

  function updateInfoPanel() {
    const titleEle = $("#skTitle"), descEle = $("#skDesc");
    if (!focusing) {
      titleEle.textContent = "";
      descEle.textContent = "点击技能查看详情。";
      return;
    }
    const n = tree.nodes.find(m => m.name === focusing);
    titleEle.textContent = infoTitle(focusing);
    descEle.textContent = (n && n.desc) || "";
  }

  // 对应零件:Skilltree.js 的 switchLearnButton：左下按钮随焦点状态切换。
  function switchLearnButton() {
    const st = focusing ? statusOf(focusing) : null;
    if (st === "selected" && !(focusing && skills[focusing] && skills[focusing].infographic)) {
      learnBtn.text.textContent = "已掌握技能";
      learnBtn.text.style.display = "";
      learnBtn.learned.style.display = "inline";
      learnBtn.normal.style.display = "none";
      learnBtn.hover.style.display = "none";
      learnBtn.down.style.display = "none";
    } else if (st === "selectable") {
      learnBtn.text.textContent = "学习";
      learnBtn.text.style.display = "";
      learnBtn.learned.style.display = "none";
      learnBtn.normal.style.display = "inline";
    } else {
      learnBtn.text.style.display = "none";
      learnBtn.learned.style.display = "none";
      learnBtn.normal.style.display = "none";
      learnBtn.hover.style.display = "none";
      learnBtn.down.style.display = "none";
    }
  }

  function learnFocused() {
    if (focusing && canLearn(focusing)) {
      activatedSkills.add(focusing);
      update();
    }
  }

  function resetSkills() {
    activatedSkills.clear();
    update();
  }

  function update() {
    if (!tree) return;
    // 先更新锁，再更新技能（与游戏 RefreshTree 的顺序一致）。
    for (const name in locks) {
      const g = gfx[name];
      if (!g) continue;
      setBg(g, isLockOpen(name) ? "unlocked" : "locked_skill", g._hover);
    }
    for (const name in skills) {
      const g = gfx[name];
      if (!g) continue;
      setBg(g, statusOf(name) || "unselected", g._hover);
    }
    $("#skillXp").textContent = `剩余洞察：${remainingXp()}`;
    const xpEle = $("#skXpNum");
    if (xpEle) xpEle.textContent = String(remainingXp());
    switchLearnButton();
    updateInfoPanel();
  }

  function render() {
    const nodes = tree.nodes;
    if (!nodes.length) {
      $("#skwrap").innerHTML = '<p class="muted">该角色暂无技能树数据。</p>';
      return;
    }
    const byName = Object.fromEntries(nodes.map(n => [n.name, n]));

    // 背景与连线（连接线由背景画承载，与零件:Skilltree.js 一致，不另画）。
    const genericBg = skillAsset("background");
    const charBg = skillAsset(`${sel.value}_background`);

    // 图层顺序（SVG 按文档顺序绘制）：bg → textbox → icon-bg → icon → focus → button。
    const parts = [];
    parts.push(svgImg(genericBg, 0, 0, WIDTH, HEIGHT));
    parts.push(svgImg(charBg, 0, 0, WIDTH, HEIGHT));

    // XP 面板（游戏 root.xp 在 (3,215)，换算同上）。
    const x_xp = WIDTH / 2 + X_XP;
    const y_xp = HEIGHT / 2 - Y_XP * yScale();
    parts.push(svgImg(skillAsset("skill_icon_textbox_white"), x_xp - XP_SIZE / 2, y_xp - XP_SIZE / 2, XP_SIZE, XP_SIZE));
    parts.push(`<text id="skXpNum" x="${x_xp}" y="${y_xp + 7}" text-anchor="middle" font-size="20" fill="white" class="unselectable">${remainingXp()}</text>`);
    parts.push(`<text x="${x_xp + 30}" y="${y_xp + 7}" font-size="15" fill="white" class="unselectable">剩余洞察</text>`);

    // 每个节点的状态底图 + 图标。
    gfx = {};
    for (const n of nodes) {
      const isLock = !n.icon;
      if (!isLock && !skills[n.name]) continue;
      if (isLock && !locks[n.name]) continue;
      const x = px(n.x), y = py(n.y);
      const size = n.infographic ? ICON_BUTTON_SIZE : (isLock ? LOCK_SIZE : ICON_BUTTON_SIZE);
      const bgName = n.infographic ? "infographic_on" : (isLock ? (isLockOpen(n.name) ? "unlocked" : "locked_skill") : statusOf(n.name) || "unselected");
      parts.push(`<g class="node" data-name="${esc(n.name)}">`);
      parts.push(`<image data-role="bg" href="${esc(skillAsset(bgName))}" x="${x - size / 2}" y="${y - size / 2}" width="${size}" height="${size}" preserveAspectRatio="xMidYMid meet"/>`);
      if (n.icon) {
        parts.push(`<image data-role="icon" href="${esc(iconUrl(n.icon))}" x="${x - ICON_SIZE / 2}" y="${y - ICON_SIZE / 2}" width="${ICON_SIZE}" height="${ICON_SIZE}" preserveAspectRatio="xMidYMid meet"/>`);
      }
      parts.push(`</g>`);
      gfx[n.name] = { bg: null, icon: null, isLock, infographic: !!n.infographic, x, y, _hover: false, _href: skillAsset(bgName) };
    }

    // 焦点框。
    parts.push(`<image data-role="skillFocus" href="${esc(skillAsset("frame"))}" width="${SKILL_FOCUS_SIZE}" height="${SKILL_FOCUS_SIZE}" style="display:none" pointer-events="none" preserveAspectRatio="xMidYMid meet"/>`);
    parts.push(`<image data-role="lockFocus" href="${esc(skillAsset("frame_octagon"))}" width="${LOCK_FOCUS_SIZE}" height="${LOCK_FOCUS_SIZE}" style="display:none" pointer-events="none" preserveAspectRatio="xMidYMid meet"/>`);

    // 底部按钮（左：学习；右：重置洞察）。
    const btnSrcs = { normal: reduxAsset("button_carny_long_normal"), hover: reduxAsset("button_carny_long_hover"), down: reduxAsset("button_carny_long_down") };
    parts.push(`<image data-role="learnLearned" pointer-events="none" href="${esc(skillAsset("skilltree_backgroundart"))}" x="${buttonLeftX}" y="${buttonY}" width="${buttonWidth}" height="${buttonHeight}" style="display:none" preserveAspectRatio="xMidYMid meet"/>`);
    for (const k of ["normal", "hover", "down"]) {
      parts.push(`<image data-role="learn${k}" pointer-events="none" href="${esc(btnSrcs[k])}" x="${buttonLeftX}" y="${buttonY}" width="${buttonWidth}" height="${buttonHeight}" preserveAspectRatio="xMidYMid meet"/>`);
    }
    parts.push(`<text data-role="learnText" pointer-events="none" class="st-button-text" x="${buttonLeftX + buttonWidth / 2}" y="${buttonY + buttonHeight / 2 + 6}" text-anchor="middle">学习</text>`);
    parts.push(`<rect data-role="learnProxy" x="${buttonLeftX}" y="${buttonY}" width="${buttonWidth}" height="${buttonHeight}" fill="transparent" style="cursor:pointer"/>`);
    for (const k of ["normal", "hover", "down"]) {
      parts.push(`<image data-role="reset${k}" pointer-events="none" href="${esc(btnSrcs[k])}" x="${buttonRightX}" y="${buttonY}" width="${buttonWidth}" height="${buttonHeight}" ${k === "normal" ? "" : 'style="display:none"'} preserveAspectRatio="xMidYMid meet"/>`);
    }
    parts.push(`<text data-role="resetText" pointer-events="none" class="st-button-text" x="${buttonRightX + buttonWidth / 2}" y="${buttonY + buttonHeight / 2 + 6}" text-anchor="middle">重置洞察</text>`);
    parts.push(`<rect data-role="resetProxy" x="${buttonRightX}" y="${buttonY}" width="${buttonWidth}" height="${buttonHeight}" fill="transparent" style="cursor:pointer"/>`);

    $("#skwrap").innerHTML =
      `<svg viewBox="0 0 ${WIDTH} ${SVG_HEIGHT}" width="100%" style="display:block;width:100%;height:auto;aspect-ratio:${WIDTH} / ${SVG_HEIGHT};background:#151923;border-radius:10px">
        ${parts.join("")}</svg>`;

    const svg = $("#skwrap svg");
    for (const name in gfx) {
      const g = gfx[name];
      const ele = svg.querySelector(`g.node[data-name="${CSS.escape(name)}"]`);
      g.bg = ele.querySelector('[data-role="bg"]');
      g.icon = ele.querySelector('[data-role="icon"]');
      ele.style.cursor = "pointer";
      ele.addEventListener("mouseenter", () => {
        g._hover = true;
        const n = byName[name];
        const base = g.infographic ? "infographic_on" : (g.isLock ? (isLockOpen(name) ? "unlocked" : "locked_skill") : statusOf(name) || "unselected");
        setBg(g, base, true);
        g.bg.parentElement.insertBefore(g.bg, g.icon || null);
      });
      ele.addEventListener("mouseleave", () => {
        g._hover = false;
        const base = g.infographic ? "infographic_on" : (g.isLock ? (isLockOpen(name) ? "unlocked" : "locked_skill") : statusOf(name) || "unselected");
        setBg(g, base, false);
      });
      ele.addEventListener("click", () => focusNode(name));
      // 游戏内双击技能按钮即学习。
      ele.addEventListener("dblclick", () => { if (canLearn(name)) { activatedSkills.add(name); update(); } });
    }

    skillFocusEle = svg.querySelector('[data-role="skillFocus"]');
    lockFocusEle = svg.querySelector('[data-role="lockFocus"]');
    learnBtn = {
      normal: svg.querySelector('[data-role="learnnormal"]'),
      hover: svg.querySelector('[data-role="learnhover"]'),
      down: svg.querySelector('[data-role="learndown"]'),
      learned: svg.querySelector('[data-role="learnLearned"]'),
      text: svg.querySelector('[data-role="learnText"]'),
    };
    resetBtn = {
      normal: svg.querySelector('[data-role="resetnormal"]'),
      hover: svg.querySelector('[data-role="resethover"]'),
      down: svg.querySelector('[data-role="resetdown"]'),
      text: svg.querySelector('[data-role="resetText"]'),
    };

    const learnProxy = svg.querySelector('[data-role="learnProxy"]');
    learnProxy.addEventListener("click", learnFocused);
    learnProxy.addEventListener("mouseenter", () => {
      if (focusing && statusOf(focusing) === "selectable") {
        learnBtn.normal.style.display = "none";
        learnBtn.hover.style.display = "inline";
      }
    });
    learnProxy.addEventListener("mouseleave", () => {
      if (focusing && statusOf(focusing) === "selectable") {
        learnBtn.hover.style.display = "none";
        learnBtn.normal.style.display = "inline";
      }
    });
    const resetProxy = svg.querySelector('[data-role="resetProxy"]');
    resetProxy.addEventListener("click", resetSkills);
    resetProxy.addEventListener("mouseenter", () => {
      resetBtn.normal.style.display = "none"; resetBtn.hover.style.display = "inline";
    });
    resetProxy.addEventListener("mouseleave", () => {
      resetBtn.hover.style.display = "none"; resetBtn.normal.style.display = "inline";
    });

    // 双击背景重置（对应 wiki 版本的 dblclick reset）。
    svg.querySelector(`image[href="${CSS.escape(skillAsset(`${sel.value}_background`))}"]`)?.addEventListener("dblclick", resetSkills);

    // 默认焦点（defaultfocus，控制器起点；缺失则取第一个根节点）。
    const defaultNode = nodes.find(n => n.defaultfocus) || nodes.find(n => n.root);
    if (defaultNode) focusNode(defaultNode.name, true);
    update();
  }

  function focusNode(name, silentScroll) {
    if (focusing !== name) {
      focusing = name;
      const g = gfx[name];
      if (g) {
        const focusEle = g.isLock ? lockFocusEle : skillFocusEle;
        const other = g.isLock ? skillFocusEle : lockFocusEle;
        const size = g.isLock ? LOCK_FOCUS_SIZE : SKILL_FOCUS_SIZE;
        focusEle.setAttribute("x", g.x - size / 2);
        focusEle.setAttribute("y", g.y - size / 2);
        focusEle.style.display = "inline";
        other.style.display = "none";
      }
    }
    if (!silentScroll) update();
    else { switchLearnButton(); updateInfoPanel(); }
  }

  // 空格键学习当前焦点（对应 wiki 版本的 onKeydownSVG）。
  if (window.__skillKeyHandler) document.removeEventListener("keydown", window.__skillKeyHandler);
  window.__skillKeyHandler = (event) => {
    if (event.code !== "Space" || !tree) return;
    if (focusing && canLearn(focusing)) learnFocused();
    event.preventDefault();
  };
  document.addEventListener("keydown", window.__skillKeyHandler);

  async function draw() {
    tree = await getJSON(`/api/viz/skilltree?character=${sel.value}`);
    buildMaps(tree.nodes || []);
    activatedSkills.clear();
    focusing = null;
    render();
  }

  sel.onchange = () => { draw(); };

  await draw();
}
/* ---------------- inventory icons ---------------- */
async function pageInventoryIcons(main) {
  main.innerHTML = `
    <div class="panel">
      <div class="row">
        <div class="tabs" style="margin:0">
          <button class="tab active" data-sort="name">按文件名</button>
          <button class="tab" data-sort="history">按加入历史</button>
        </div>
        <input id="iq" placeholder="搜索文件名 / 中文名 / 英文名…" style="width:250px">
        <span class="muted" id="icount"></span>
      </div>
      <p class="muted" style="margin:8px 0 0">点击图标查看历代版本（内容寻址历史，来自 images-sync manifest）。</p>
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

  const st = { sort: "name", q: "", page: 0, size: 120, loading: false, done: false, lastGroup: null };

  const loadMore = async () => {
    if (st.loading || st.done) return;
    st.loading = true;
    try {
      const p = new URLSearchParams({ sort: st.sort, q: st.q, page: st.page, page_size: st.size });
      const r = await getJSON(`/api/data/inventoryicons?${p}`);
      $("#icount").textContent = `共 ${r.total} 个图标 · 数据源 build ${r.latest_build ?? "—"}`;
      const grid = $("#igrid");
      for (const it of r.items) {
        const gk = groupKey(it);
        if (gk !== st.lastGroup) {
          st.lastGroup = gk;
          const label = st.sort === "name" ? gk
            : `Build ${it.first_build}${it.first_synced_at ? ` · ${fmtDate(it.first_synced_at)}` : ""}`;
          grid.insertAdjacentHTML("beforeend", `<div class="icon-divider">${esc(label)}</div>`);
        }
        const names = [it.name_zh, it.name_en].filter(Boolean).join(" / ");
        grid.insertAdjacentHTML("beforeend", `
          <figure class="icon-card" data-file="${esc(it.file)}">
            <img loading="lazy" width="64" height="64"
              src="/static/split/inventoryimages/${encodeURIComponent(it.file)}" alt="${esc(it.file)}">
            <figcaption>
              <code>${esc(it.file.replace(/\.png$/, ""))}</code>
              ${names ? `<span class="muted">${esc(names)}</span>` : ""}
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

  main.querySelectorAll(".tab").forEach(b => b.onclick = () => {
    if (b.dataset.sort === st.sort) return;
    st.sort = b.dataset.sort;
    main.querySelectorAll(".tab").forEach(x => x.classList.toggle("active", x === b));
    reset();
  });

  let deb;
  $("#iq").oninput = (e) => {
    clearTimeout(deb);
    deb = setTimeout(() => { st.q = e.target.value.trim(); reset(); }, 250);
  };

  const io = new IntersectionObserver((es) => {
    if (es.some(e => e.isIntersecting)) loadMore();
  }, { rootMargin: "300px" });
  io.observe($("#isentinel"));

  // 历代版本抽屉
  const overlay = $("#ivOverlay");
  $("#ivClose").onclick = () => { overlay.style.display = "none"; };
  overlay.onclick = (e) => { if (e.target === overlay) overlay.style.display = "none"; };
  $("#igrid").addEventListener("click", async (e) => {
    const card = e.target.closest(".icon-card");
    if (!card) return;
    const file = card.dataset.file;
    $("#ivTitle").textContent = file;
    $("#ivList").innerHTML = '<p class="muted">加载中…</p>';
    overlay.style.display = "flex";
    try {
      const r = await getJSON(`/api/data/inventoryicons/versions?file=${encodeURIComponent(file)}`);
      $("#ivList").innerHTML = r.versions.map((v, i) => `
        <div class="icon-ver">
          <img loading="lazy" width="64" height="64" src="/static/objects/${esc(v.hash)}" alt="">
          <div>
            <b>Build ${esc(v.build)}</b>${v.synced_at ? `<span class="muted"> · ${fmtDate(v.synced_at)}</span>` : ""}
            ${i === 0 && r.present ? ' <span class="badge b-success">当前</span>' : ""}
          </div>
          <code class="muted" style="margin-left:auto">${esc(v.hash.slice(0, 12))}…</code>
        </div>`).join("") || '<p class="muted">无历史记录。</p>';
    } catch (err) {
      $("#ivList").innerHTML = `<p style="color:var(--err)">加载失败：${esc(err.message)}</p>`;
    }
  });
}

/* ---------------- constants ---------------- */
async function pageConstants(main) {
  main.innerHTML = `
    <div class="panel">
      <div class="row">
        <input id="cq" placeholder="搜索 TUNING 键或值…" style="width:280px">
        <span class="muted" id="ccount"></span>
      </div>
    </div>
    <div class="panel"><div id="ctable"></div><div id="cpager"></div></div>`;
  const st = { q: "", page: 0, size: 80 };
  const load = async () => {
    const p = new URLSearchParams({ q: st.q, page: st.page, page_size: st.size });
    const r = await getJSON(`/api/data/constants?${p}`);
    $("#ccount").textContent = `共 ${r.total} 个常量`;
    $("#ctable").innerHTML = `<table><thead><tr><th style="width:340px">键</th><th>值</th></tr></thead><tbody>
      ${r.items.map(x => `<tr><td><code>${esc(x.key)}</code></td><td>${esc(x.value)}</td></tr>`).join("")}
      </tbody></table>`;
    $("#cpager").innerHTML = "";
    $("#cpager").appendChild(pager(r.total, r.page, r.page_size, (pg) => { st.page = pg; load(); }));
  };
  let deb;
  $("#cq").oninput = (e) => { clearTimeout(deb); deb = setTimeout(() => { st.q = e.target.value.trim(); st.page = 0; load(); }, 250); };
  await load();
}

/* ---------------- animation assets ---------------- */
async function pageAnimAssets(main) {
  const state = {
    q: "",
    items: [],
    prefabFile: null,
    prefabItems: [],
    animFiles: [],
    relatedFiles: [],
    fileInfo: [],
    buildFiles: {},
    selectedFile: null,
    selectedBank: null,
    selectedAnim: null,
    info: null,
    hiddenSymbols: new Set(),
    symbolBuilds: {},
    remaps: {},
    remapChoice: {},
    remapOptOut: new Set(),
    clothingSymbols: new Set(),
    disabledBuilds: new Set(),
    candidateBuilds: [],
    extraBuilds: new Set(),
    skins: [],
    selectedSkin: null,
    previewTimer: null,
  };

  async function apiJSON(url) { return getJSON(url); }

  async function search() {
    const kw = state.q.trim();
    const url = `/api/anim/assets/prefabs${kw ? `?q=${encodeURIComponent(kw)}` : ""}`;
    const r = await apiJSON(url);
    state.items = r.items || [];
    renderList();
  }

  function renderList() {
    main.innerHTML = `
      <div class="panel">
        <h2>动画素材检索</h2>
        <div class="row">
          <input id="assetQ" placeholder="搜索 prefab 名或文件名，如 hound / axe…" style="width:320px" value="${esc(state.q)}">
          <button class="btn" id="assetSearch">搜索</button>
        </div>
        <p class="muted" id="assetCount"></p>
      </div>
      <div class="panel" id="assetResults"></div>`;
    $("#assetCount").textContent = `共 ${state.items.length} 条`;
    const groups = {};
    for (const item of state.items) {
      const key = item.prefab_file || "?";
      (groups[key] = groups[key] || []).push(item);
    }
    $("#assetResults").innerHTML = Object.entries(groups).map(([file, items]) => `
      <div class="row" style="justify-content:space-between;align-items:center">
        <div>
          <code>${esc(file)}</code>
          ${items.map(i => `<span class="chip">${esc(i.prefab_name || "?")}${i.skin_count ? ` <span class="muted">· skins ${i.skin_count}</span>` : ""}</span>`).join("")}
          <span class="muted">(${(items[0].anims || []).length} 动画文件 / ${(items[0].related_files || []).length} build 文件)</span>
        </div>
        <button class="btn secondary" data-open="${esc(file)}">进入预览</button>
      </div>`).join("") || '<p class="muted">未找到匹配 prefab。</p>';
    $("#assetResults").querySelectorAll("[data-open]").forEach(btn => {
      btn.onclick = () => openPrefab(btn.dataset.open);
    });
    $("#assetSearch").onclick = search;
    $("#assetQ").addEventListener("input", e => { state.q = e.target.value; });
    $("#assetQ").addEventListener("keydown", e => { if (e.key === "Enter") search(); });
  }

  async function openPrefab(file) {
    state.prefabFile = file;
    state.prefabItems = [];
    state.animFiles = [];
    state.relatedFiles = [];
    state.fileInfo = [];
    state.buildFiles = {};
    state.hiddenSymbols.clear();
    state.symbolBuilds = {};
    state.remaps = {};
    state.remapChoice = {};
    state.remapOptOut = new Set();
    state.clothingSymbols.clear();
    state.disabledBuilds.clear();
    state.candidateBuilds = [];
    state.extraBuilds.clear();
    state.skins = [];
    state.selectedSkin = null;
    const r = await apiJSON(`/api/anim/assets/prefab/${encodeURIComponent(file)}`);
    state.prefabItems = r.items || [];
    state.fileInfo = r.files || [];
    state.buildFiles = r.build_files || {};
    state.skins = r.skins || [];
    for (const item of state.prefabItems) {
      for (const a of (item.anims || [])) {
        const p = a.normalized;
        if (!state.animFiles.includes(p)) state.animFiles.push(p);
      }
      for (const rel of (item.related_files || [])) {
        if (!state.relatedFiles.includes(rel)) state.relatedFiles.push(rel);
      }
    }

    // Directly enter animation detail: pick the first available animation.
    for (const path of state.animFiles) {
      const c = fileContent(path);
      const banks = c.banks || [];
      const animations = c.animations || [];
      if (banks.length && animations.length) {
        await openAnimation(path, banks[0], animations[0]);
        return;
      }
    }
    // Fallback: no animation found.
    renderPrefab();
  }

  function fileContent(path) {
    const f = state.fileInfo.find(x => x.path === path);
    const raw = (f && f.content) || state.buildFiles[path] || {};
    if (raw.content && (raw.content.banks || raw.content.builds)) return raw.content;
    return raw;
  }

  function renderPrefab() {
    const animFiles = state.animFiles.filter(path => {
      const c = fileContent(path);
      return (c.banks || []).length > 0 || (c.animations || []).length > 0;
    });
    const relatedFiles = state.relatedFiles;
    main.innerHTML = `
      <div class="panel">
        <div class="row">
          <button class="btn secondary" id="backToList">返回搜索</button>
          <h2 style="margin:0"><code>${esc(state.prefabFile)}</code></h2>
        </div>
      </div>
      <div class="card-row">
        <div class="card"><div class="num">${animFiles.length}</div><span class="muted">动画文件</span></div>
        <div class="card"><div class="num">${relatedFiles.length}</div><span class="muted">Build/package 文件</span></div>
      </div>
      <div class="panel">
        <h2>Animations 按文件</h2>
        ${animFiles.map(path => {
          const c = fileContent(path);
          const banks = c.banks || [];
          const animations = c.animations || [];
          const banksHtml = banks.map(b => `
            <div style="margin-left:12px">
              <b>${esc(b)}</b>
              <div style="margin-left:12px">${animations.map(a => `
                <button class="btn link" data-play="${esc(path)}|${esc(b)}|${esc(a)}">${esc(a)}</button>
              `).join("") || '<span class="muted">无动画</span>'}</div>
            </div>`).join("");
          return `<div class="row" style="align-items:flex-start"><code>${esc(path)}</code><div>${banksHtml || '<span class="muted">无 banks</span>'}</div></div>`;
        }).join("") || '<p class="muted">无动画文件</p>'}
      </div>
      <div class="panel">
        <h2>Builds 与 Symbols / Atlases</h2>
        ${relatedFiles.concat(animFiles).map(path => {
          const c = fileContent(path);
          if (!c.builds && !c.symbols && !c.atlases) return "";
          const disabled = state.disabledBuilds.has(path);
          return `<div class="row" style="align-items:flex-start">
            <label style="display:flex;align-items:center;gap:6px"><input type="checkbox" data-build-toggle="${esc(path)}" ${disabled ? "" : "checked"}> <code>${esc(path)}</code></label>
            <div class="muted">
              ${(c.builds || []).map(x => `build: ${esc(x)}`).join(", ")}
              <div>symbols: ${(c.symbols || []).slice(0, 50).map(esc).join(", ") || "—"}</div>
              <div>atlases: ${(c.atlases || []).map(esc).join(", ") || "—"}</div>
            </div>
          </div>`;
        }).join("") || '<p class="muted">无 build 文件</p>'}
      </div>`;

    $("#backToList").onclick = () => { state.prefabFile = null; renderList(); };
    main.querySelectorAll("[data-play]").forEach(btn => {
      btn.onclick = () => {
        const [file, bank, anim] = btn.dataset.play.split("|");
        openAnimation(file, bank, anim);
      };
    });
    main.querySelectorAll("[data-build-toggle]").forEach(chk => {
      chk.onchange = () => {
        const path = chk.dataset.buildToggle;
        if (chk.checked) state.disabledBuilds.delete(path);
        else state.disabledBuilds.add(path);
      };
    });
  }

  async function openAnimation(file, bank, anim) {
    state.selectedFile = file;
    state.selectedBank = bank;
    state.selectedAnim = anim;
    state.info = null;
    const files = state.relatedFiles.concat(state.animFiles.filter(f => f !== file)).concat([file]);
    const skinQ = state.selectedSkin
      ? `&skin_zip=${encodeURIComponent(state.selectedSkin.zip || "")}` +
        (state.selectedSkin.dyn ? `&skin_dyn=${encodeURIComponent(state.selectedSkin.dyn)}` : "")
      : "";
    const url = `/api/anim/assets/info?files=${encodeURIComponent(files.join(","))}&bank=${encodeURIComponent(bank)}&animation=${encodeURIComponent(anim)}${skinQ}`;
    state.info = await apiJSON(url);
    // Fetch static AnimState override candidates (anim-remap-index) for the
    // animation's symbols; degrade silently when the artifact is missing.
    state.remaps = {};
    try {
        const syms = (state.info.symbols || []).join(",");
        const r = await apiJSON(`/api/anim/assets/remaps${syms ? `?symbols=${encodeURIComponent(syms)}` : ""}`);
        state.remaps = r.symbols || {};
        for (const entries of Object.values(state.remaps)) {
          for (const e of entries) e.source = "tier_a";
        }
    } catch (e) { /* no remap artifact — selector stays hidden */ }
    renderAnimationDetail();
  }

  function renderAnimationDetail() {
    const info = state.info;
    if (!info) return;
    const symbolSet = info.symbols || [];
    const builds = info.builds || [];
    const frames = info.frames || [];

    main.innerHTML = `
      <div class="anim-detail-grid">
        <div class="anim-col-left">
          <div class="panel" style="margin:0">
            <div class="row" style="justify-content:space-between;align-items:center">
              <h3 style="margin:0">Skin</h3>
              <span class="muted">${state.skins.length} 可用</span>
            </div>
            <select id="skinSel" style="width:100%">
              <option value="">无皮肤</option>
              ${state.skins.map(s => `<option value="${esc(s.skin)}" ${state.selectedSkin && state.selectedSkin.skin === s.skin ? "selected" : ""}>${esc(s.skin)}</option>`).join("")}
            </select>
          </div>
          <div class="panel" style="margin:0">
            <h3 style="margin:0 0 4px">衣物覆盖</h3>
            <input id="clothingName" list="clothingNames" placeholder="衣物名，如 body_onepiece3_beach" style="width:100%">
            <datalist id="clothingNames"></datalist>
            <input id="clothingChar" list="characterNames" placeholder="角色（可空 = default）" style="width:100%;margin-top:4px">
            <datalist id="characterNames">
              <option value="default">default</option>
              ${["wilson","willow","wolfgang","wickerbottom","wes","maxwell","waxwell","wagstaff","walter","wanda","warly","webber","wendy","wortox","wormwood","wurt","woodie","wathgrithr","winona","wx78","walani","woodlegs","wilba","woodcutter"].map(c => `<option value="${c}">${c}</option>`).join("")}
            </datalist>
            <div class="row" style="margin-top:4px;gap:4px">
              <button class="btn" id="applyClothing">应用覆盖</button>
              <button class="btn link" id="clearClothing">清除</button>
            </div>
            <div id="clothingMsg" class="muted" style="font-size:12px;margin-top:2px"></div>
          </div>
          <div class="panel anim-side-panel">
            <div class="row" style="justify-content:space-between;align-items:center">
              <h3 style="margin:0">Animations</h3>
              <button class="btn link" id="collapseAnims">折叠全部</button>
            </div>
            <div id="leftAnims" style="overflow-y:auto;flex:1;min-height:0"></div>
          </div>
          <div class="panel anim-side-panel">
            <div class="row" style="justify-content:space-between;align-items:center">
              <h3 style="margin:0">Builds</h3>
              <button class="btn link" id="findMissingBuilds">补齐缺失 Symbol</button>
            </div>
            <div id="leftBuilds" style="overflow-y:auto;flex:1;min-height:0"></div>
            <div class="row" style="justify-content:space-between;align-items:center;margin-top:6px">
              <h4 style="margin:0">候选 Builds</h4>
              <span style="display:flex;gap:4px">
                <button class="btn link" id="candidateAll">全选</button>
                <button class="btn link" id="candidateNone">取消全选</button>
                <button class="btn link" id="candidateInvert">反选</button>
              </span>
            </div>
            <div id="candidateBuilds" style="overflow-y:auto;max-height:35%;min-height:0"></div>
          </div>
        </div>
        <div class="panel anim-col-center">
          <div class="row" style="justify-content:space-between;align-items:center">
            <button class="btn secondary" id="backToPrefab">返回搜索</button>
            <b><code>${esc(info.bank)} / ${esc(info.animation)}</code></b>
          </div>
          <div id="animPreview" style="flex:1;width:100%;height:100%;min-height:0;overflow:auto;display:flex;align-items:center;justify-content:center;background:repeating-conic-gradient(#fff 0 25%, #eee 0 50%) 0 0/16px 16px;border:1px solid var(--border);margin:8px 0;position:relative"></div>
          <div id="frameCounter" style="text-align:center;margin-bottom:6px"></div>
          <div class="row" style="justify-content:center;gap:8px">
            <button class="btn" id="exportGif">导出 GIF</button>
            <button class="btn secondary" id="exportPng">导出 PNG 序列</button>
          </div>
        </div>
        <div class="anim-col-right">
          <div class="panel anim-side-panel">
            <h3>当前帧详情</h3>
            <div style="margin-bottom:6px">
              <input type="range" id="frameRange" min="0" max="${Math.max(0, frames.length - 1)}" value="0" style="width:100%">
            </div>
            <div id="frameDetail" style="overflow-y:auto;flex:1;min-height:0"></div>
          </div>
          <div class="panel anim-side-panel">
            <div class="row" style="justify-content:space-between;align-items:center">
              <h3 style="margin:0">Symbol Dependencies</h3>
              <button class="btn link" id="collapseSymbols">折叠全部</button>
            </div>
            <div id="symbolDeps" style="overflow-y:auto;flex:1;min-height:0"></div>
          </div>
        </div>
      </div>`;

    $("#backToPrefab").onclick = () => renderList();

    // Skin switch: re-fetch info + preview with the selected skin package.
    const skinSel = $("#skinSel");
    if (skinSel) {
      skinSel.onchange = () => {
        const v = skinSel.value;
        state.selectedSkin = v ? (state.skins.find(s => s.skin === v) || null) : null;
        // Re-load the current animation so Symbol Dependencies and the
        // preview both pick up the skin build (kept across animation switches).
        openAnimation(state.selectedFile, state.selectedBank, state.selectedAnim);
      };
    }

    // Left: animation list
    const animList = [];
    for (const path of state.animFiles) {
      const c = fileContent(path);
      for (const b of (c.banks || [])) {
        for (const a of (c.animations || [])) {
          animList.push({ file: path, bank: b, anim: a });
        }
      }
    }
    const groupedAnims = {};
    for (const x of animList) {
      (groupedAnims[x.file] = groupedAnims[x.file] || []).push(x);
    }
    $("#leftAnims").innerHTML = Object.entries(groupedAnims).map(([file, items]) => `
      <details ${file === state.selectedFile ? "open" : ""} style="margin-bottom:2px">
        <summary style="cursor:pointer;white-space:nowrap;overflow:hidden;text-overflow:ellipsis"><code>${esc(file)}</code></summary>
        <div style="margin-left:10px">
          ${items.map(x => {
            const active = x.file === state.selectedFile && x.bank === state.selectedBank && x.anim === state.selectedAnim;
            const label = `${x.bank} / ${x.anim}`;
            return `<button class="btn link anim-item ${active ? 'active' : ''}" data-switch="${esc(x.file)}|${esc(x.bank)}|${esc(x.anim)}" title="${esc(label)}">${esc(label)}</button>`;
          }).join("")}
        </div>
      </details>`).join("") || '<p class="muted">无动画</p>';
    $("#leftAnims").querySelectorAll("[data-switch]").forEach(btn => {
      btn.onclick = () => {
        const [file, bank, anim] = btn.dataset.switch.split("|");
        openAnimation(file, bank, anim);
      };
    });

    // Left: build list — each build expands to its full symbol list with a
    // client-side filter; the checkbox stays on the summary row (clicks on it
    // must not toggle the disclosure).
    $("#leftBuilds").innerHTML = builds.map(b => {
      const disabled = state.disabledBuilds.has(b.file);
      const syms = b.symbols || [];
      return `<details data-build-details="${esc(b.file)}" style="padding:1px 0">
        <summary class="list-row" style="cursor:pointer" title="${esc(syms.join(", "))}">
          <input type="checkbox" data-build-toggle="${esc(b.file)}" ${disabled ? "" : "checked"}>
          <code class="list-name" title="${esc(b.file)}">${esc(b.file)}</code>
          <span class="list-meta">${syms.length}s / ${(b.atlases || []).length}a</span>
        </summary>
        <div style="margin:4px 0 6px 18px">
          <input type="text" placeholder="过滤 symbol…" data-build-filter="${esc(b.file)}" style="width:100%">
          <div class="muted" style="font-size:12px;margin-top:2px">atlases: ${(b.atlases || []).map(esc).join(", ") || "—"}</div>
          <div data-build-symbols style="display:flex;flex-wrap:wrap;gap:4px;margin-top:4px;max-height:180px;overflow-y:auto">
            ${syms.map(s => `<span class="chip" style="font-size:11px">${esc(s)}</span>`).join("") || '<span class="muted">无 symbol</span>'}
          </div>
        </div>
      </details>`;
    }).join("") || '<p class="muted">无 build</p>';
    $("#leftBuilds").querySelectorAll("[data-build-toggle]").forEach(chk => {
      // Clicking the checkbox must not also toggle the <details> disclosure.
      chk.addEventListener("click", e => e.stopPropagation());
      chk.onchange = () => {
        const path = chk.dataset.buildToggle;
        if (chk.checked) state.disabledBuilds.delete(path);
        else state.disabledBuilds.add(path);
        renderSymbolDeps();
        schedulePreviewRefresh();
      };
    });
    // Symbol filter inside each expanded build.
    $("#leftBuilds").querySelectorAll("[data-build-filter]").forEach(inp => {
      inp.oninput = () => {
        const q = inp.value.trim().toLowerCase();
        const details = inp.closest("details");
        const chips = details.querySelectorAll("[data-build-symbols] .chip");
        let shown = 0;
        chips.forEach(c => {
          const hit = !q || c.textContent.toLowerCase().includes(q);
          c.style.display = hit ? "" : "none";
          if (hit) shown++;
        });
        let counter = details.querySelector("[data-build-filter-count]");
        if (!counter) {
          counter = document.createElement("span");
          counter.className = "muted";
          counter.style.fontSize = "12px";
          counter.setAttribute("data-build-filter-count", "");
          details.querySelector("[data-build-symbols]").before(counter);
        }
        counter.textContent = q ? `${shown} / ${chips.length}` : "";
      };
    });

    // One-click find build packages for missing symbols
    const missingSymbols = symbolSet.filter(sym =>
      !builds.some(b => (b.symbols || []).some(x => x.toLowerCase() === sym.toLowerCase()))
    );

    const renderCandidates = () => {
      const list = state.candidateBuilds || [];
      $("#candidateBuilds").innerHTML = list.length === 0
        ? '<p class="muted">未搜索候选 build</p>'
        : list.map(b => {
            const checked = state.extraBuilds.has(b.file);
            const disabled = state.disabledBuilds.has(b.file);
            return `<div class="list-row">
              <input type="checkbox" data-candidate-toggle="${esc(b.file)}" ${checked ? "checked" : ""} ${disabled ? "disabled" : ""}>
              <code class="list-name" title="${esc(b.file)}">${esc(b.file)}</code>
              <span class="list-meta" title="${esc((b.matched_symbols || []).join(", "))}">${(b.matched_symbols || []).join(",")}</span>
            </div>`;
          }).join("");
      $("#candidateBuilds").querySelectorAll("[data-candidate-toggle]").forEach(chk => {
        chk.onchange = () => {
          const path = chk.dataset.candidateToggle;
          if (chk.checked) state.extraBuilds.add(path);
          else state.extraBuilds.delete(path);
          renderCandidates();
          renderSymbolDeps();
          schedulePreviewRefresh();
        };
      });
    };

    const loadCandidates = async () => {
      if (missingSymbols.length === 0) return;
      $("#candidateBuilds").innerHTML = '<p class="muted">搜索中…</p>';
      try {
        const r = await apiJSON(`/api/anim/assets/find-builds?symbols=${encodeURIComponent(missingSymbols.join(","))}`);
        state.candidateBuilds = r.builds || [];
        // Auto-check all found candidates so missing symbols are filled immediately.
        for (const b of state.candidateBuilds) state.extraBuilds.add(b.file);
        renderCandidates();
        renderSymbolDeps();
        schedulePreviewRefresh();
      } catch (e) {
        $("#candidateBuilds").innerHTML = `<p style="color:var(--err)">失败：${esc(e.message)}</p>`;
      }
    };
    $("#findMissingBuilds").onclick = loadCandidates;
    renderCandidates();

    // Select all / none / invert for candidate builds
    $("#candidateAll").onclick = () => {
      for (const b of state.candidateBuilds) state.extraBuilds.add(b.file);
      renderCandidates();
      renderSymbolDeps();
      schedulePreviewRefresh();
    };
    $("#candidateNone").onclick = () => {
      for (const b of state.candidateBuilds) state.extraBuilds.delete(b.file);
      renderCandidates();
      renderSymbolDeps();
      schedulePreviewRefresh();
    };
    $("#candidateInvert").onclick = () => {
      for (const b of state.candidateBuilds) {
        if (state.extraBuilds.has(b.file)) state.extraBuilds.delete(b.file);
        else state.extraBuilds.add(b.file);
      }
      renderCandidates();
      renderSymbolDeps();
      schedulePreviewRefresh();
    };

    // Collapse all two-level lists
    $("#collapseAnims").onclick = () => {
      main.querySelectorAll("#leftAnims details").forEach(d => d.open = false);
    };
    $("#collapseSymbols").onclick = () => {
      main.querySelectorAll("#symbolDeps details").forEach(d => d.open = false);
    };

    // Right: frame detail renderer
    const renderFrameDetail = (idx) => {
      const frame = frames[Math.min(idx, Math.max(0, frames.length - 1))];
      const range = $("#frameRange");
      if (range) range.value = String(idx);
      if (!frame) { $("#frameDetail").innerHTML = '<p class="muted">无帧数据</p>'; return; }
      $("#frameDetail").innerHTML = `
        <p class="muted">Frame ${frame.idx} · ${frame.elements.length} elements · ${frame.events.length} events</p>
        ${frame.elements.length === 0 ? '<p class="muted">无元素</p>' : frame.elements.map((e, i) => `
          <div class="row" style="justify-content:space-between;gap:6px;padding:2px 0">
            <span>#${i} <code>${esc(e.symbol)}</code></span>
            <span class="muted">${esc(e.layer)} z=${e.z}</span>
          </div>`).join("")}
      `;
    };
    $("#frameRange").oninput = (e) => {
      const idx = parseInt(e.target.value) || 0;
      renderFrameDetail(idx);
      // Also seek preview if already loaded? Not implemented for simplicity.
    };

    // Right: symbol dependencies (re-rendered on state changes to stay consistent)
    const isTierA = (e) => e.source !== "clothing";
    const providersFor = (sym) =>
      builds.concat((state.candidateBuilds || []).filter(b => state.extraBuilds.has(b.file)))
        .filter(b => (b.symbols || []).some(x => x.toLowerCase() === sym.toLowerCase()));
    const enabledProvidersFor = (sym) =>
      providersFor(sym).filter(b => !state.disabledBuilds.has(b.file));
    const autoRemapIdx = (sym) => {
      if (state.hiddenSymbols.has(sym)) return -1;
      if (Object.prototype.hasOwnProperty.call(state.remapChoice, sym)) return -1;
      if (state.remapOptOut.has(sym)) return -1;
      if (enabledProvidersFor(sym).length > 0) return -1;
      return (state.remaps[sym] || []).findIndex(e => isTierA(e) && e.confidence === "static");
    };
    const effectiveRemapIdx = (sym) => {
      if (Object.prototype.hasOwnProperty.call(state.remapChoice, sym)) {
        const idx = state.remapChoice[sym];
        return idx >= 0 && idx < (state.remaps[sym] || []).length ? idx : -1;
      }
      return autoRemapIdx(sym);
    };

    const renderSymbolDeps = () => {
      const allBuilds = builds.concat((state.candidateBuilds || []).filter(b => state.extraBuilds.has(b.file)));
      const symHtml = symbolSet.map(sym => {
        const providers = allBuilds.filter(b => (b.symbols || []).some(x => x.toLowerCase() === sym.toLowerCase()));
        const hidden = state.hiddenSymbols.has(sym);
        const chosen = state.symbolBuilds[sym] || "";
        const defaultChosen = providers.find(b => !state.disabledBuilds.has(b.file));
        const effectiveChosen = chosen || (defaultChosen ? defaultChosen.file : "");
        const remaps = state.remaps[sym] || [];
        const autoIdx = autoRemapIdx(sym);
        const activeIdx = effectiveRemapIdx(sym);
        const remapHtml = remaps.length === 0 ? "" : `
                <div style="margin:3px 0;display:flex;align-items:center;gap:6px">
                  <span class="muted" style="font-size:11px">↳ 映射候选</span>
                  <select data-sym-remap="${esc(sym)}" style="flex:1;font-size:12px;padding:4px 6px">
                    <option value="" ${activeIdx < 0 ? "selected" : ""}>— 不覆盖 —</option>
                    ${remaps.map((r, i) => `<option value="${i}" ${activeIdx === i ? "selected" : ""}>${i === autoIdx ? "auto · " : ""}${r.confidence === "resolved" ? "≈ " : ""}${esc(r.build || "*")} › ${esc(r.src_symbol)} · ${esc(r.api || "")}${r.prefabs && r.prefabs.length ? ` (${esc(r.prefabs[0])})` : ""}</option>`).join("")}
                  </select>
                </div>
                ${autoIdx >= 0 ? '<div class="muted" style="font-size:11px">无同名 provider，自动采用 static 映射</div>' : ""}`;
        return `
          <div class="list-row">
            <input type="checkbox" data-sym-toggle="${esc(sym)}" ${hidden ? "" : "checked"}>
            <details style="flex:1;min-width:0">
              <summary style="cursor:pointer;display:flex;align-items:center;gap:4px">
                <code class="list-name" title="${esc(sym)}">${esc(sym)}</code>${hidden ? '<span class="badge b-warn">hidden</span>' : ""}${autoIdx >= 0 ? '<span class="badge b-ok">auto-map</span>' : ""}
              </summary>
              <div style="margin-left:10px;padding:2px 0">
                ${providers.length === 0 ? '<span class="muted">missing — no build provides this symbol</span>' : providers.map(b => `
                  <label style="display:flex;align-items:center;gap:6px;padding:1px 0;font-size:12px">
                    <input type="radio" name="sym-${esc(sym)}" value="${esc(b.file)}"
                      data-sym-build="${esc(sym)}" ${effectiveChosen === b.file ? "checked" : ""}
                      ${state.disabledBuilds.has(b.file) ? "disabled" : ""}>
                    <code class="list-name" title="${esc(b.file)}">${esc(b.file)}</code>
                    <span class="list-meta">${esc(b.name)}${b.skin ? " · skin" : ""}</span>
                  </label>`).join("")}
                ${remapHtml}
              </div>
            </details>
          </div>`;
      }).join("");
      $("#symbolDeps").innerHTML = symHtml || '<p class="muted">无 symbol 依赖</p>';

      main.querySelectorAll("[data-sym-toggle]").forEach(chk => {
        chk.onchange = () => {
          const sym = chk.dataset.symToggle;
          if (chk.checked) state.hiddenSymbols.delete(sym);
          else state.hiddenSymbols.add(sym);
          renderSymbolDeps();
          schedulePreviewRefresh();
        };
      });
      main.querySelectorAll("[data-sym-build]").forEach(radio => {
        radio.onchange = () => {
          const sym = radio.dataset.symBuild;
          state.symbolBuilds[sym] = radio.value;
          delete state.remapChoice[sym];
          state.remapOptOut.delete(sym);
          renderSymbolDeps();
          schedulePreviewRefresh();
        };
      });
      main.querySelectorAll("[data-sym-remap]").forEach(sel => {
        sel.onchange = () => {
          const sym = sel.dataset.symRemap;
          if (sel.value === "") {
            delete state.remapChoice[sym];
            state.remapOptOut.add(sym);
          } else {
            state.remapOptOut.delete(sym);
            state.remapChoice[sym] = parseInt(sel.value);
          }
          renderSymbolDeps();
          schedulePreviewRefresh();
        };
      });
    };
    renderSymbolDeps();

    // Clothing overrides (Tier-B): resolve a CLOTHING[name] entry into symbol
    // overrides and merge them into the same remap-choice mechanism.
    apiJSON("/api/anim/assets/clothing-overrides").then(r => {
      const dl = $("#clothingNames");
      if (dl) dl.innerHTML = (r.items || []).map(i => `<option value="${esc(i.name)}">${esc(i.type || "")} · ${i.symbols} symbols</option>`).join("");
    }).catch(() => {});
    $("#applyClothing").onclick = async () => {
      const name = $("#clothingName").value.trim().toLowerCase();
      if (!name) { $("#clothingMsg").textContent = "请输入衣物名"; return; }
      const ch = $("#clothingChar").value.trim();
      try {
        const r = await apiJSON(`/api/anim/assets/clothing-overrides?name=${encodeURIComponent(name)}${ch ? `&character=${encodeURIComponent(ch)}` : ""}`);
        const overrides = r.overrides || {};
        for (const sym of state.clothingSymbols) {
          delete state.remaps[sym];
          delete state.remapChoice[sym];
        }
        state.clothingSymbols.clear();
        for (const [sym, src] of Object.entries(overrides)) {
          state.remaps[sym] = [{ build: r.build, src_symbol: src, api: "OverrideSkinSymbol", prefabs: [name], source: "clothing" }];
          state.remapChoice[sym] = 0;
          state.remapOptOut.delete(sym);
          state.clothingSymbols.add(sym);
        }
        $("#clothingMsg").textContent = `已应用 ${Object.keys(overrides).length} 个覆盖（build: ${r.build}）`;
        renderSymbolDeps();
        schedulePreviewRefresh();
      } catch (e) {
        $("#clothingMsg").textContent = "失败：" + e.message;
      }
    };
    $("#clearClothing").onclick = () => {
      for (const sym of state.clothingSymbols) {
        delete state.remaps[sym];
        delete state.remapChoice[sym];
      }
      state.clothingSymbols.clear();
      $("#clothingMsg").textContent = "已清除衣物覆盖";
      renderSymbolDeps();
      schedulePreviewRefresh();
    };

    const buildRenderParams = (format) => {
      const extra = Array.from(state.extraBuilds).filter(f => f !== state.selectedFile);
      const files = extra.concat(state.relatedFiles).concat(state.animFiles.filter(f => f !== state.selectedFile)).concat([state.selectedFile]);
      const params = new URLSearchParams({
        files: files.join(","),
        bank: state.selectedBank,
        animation: state.selectedAnim,
        format,
      });
      if (state.disabledBuilds.size) params.set("disabled_builds", Array.from(state.disabledBuilds).join(","));
      if (state.hiddenSymbols.size) params.set("hidden_symbols", Array.from(state.hiddenSymbols).join(","));
      const sb = Object.entries(state.symbolBuilds).map(([s,b]) => `${s}:${b}`).join(",");
      if (sb) params.set("symbol_builds", sb);
      const so = [];
      const seen = new Set();
      for (const sym of symbolSet) {
        if (state.hiddenSymbols.has(sym)) continue;
        const idx = effectiveRemapIdx(sym);
        if (idx < 0) continue;
        const r = (state.remaps[sym] || [])[idx];
        if (r) {
          so.push(`${sym}:${r.build || ""}:${r.src_symbol}`);
          seen.add(sym);
        }
      }
      for (const [sym, idx] of Object.entries(state.remapChoice)) {
        if (seen.has(sym)) continue;
        const r = (state.remaps[sym] || [])[idx];
        if (r) so.push(`${sym}:${r.build || ""}:${r.src_symbol}`);
      }
      if (so.length) params.set("symbol_overrides", so.join(","));
      if (state.selectedSkin) {
        params.set("skin_zip", state.selectedSkin.zip || "");
        if (state.selectedSkin.dyn) params.set("skin_dyn", state.selectedSkin.dyn);
      }
      return params.toString();
    };

    // Center preview with PNG frames + wheel zoom
    const previewArea = $("#animPreview");
    let zoom = 1;
    previewArea.addEventListener("wheel", (e) => {
      e.preventDefault();
      zoom = Math.min(4, Math.max(0.2, zoom * (e.deltaY > 0 ? 0.9 : 1.1)));
      const img = $("#animFrame");
      if (img) img.style.transform = `scale(${zoom})`;
    }, { passive: false });

    const startPreview = async () => {
      previewArea.innerHTML = '<p class="muted">渲染中…</p>';
      try {
        const res = await fetch(`/api/anim/assets/preview?${buildRenderParams("gif")}`);
        if (!res.ok) throw new Error(await res.text());
        const data = await res.json();
        const imgs = (data.frames || []).map(f => f.data);
        if (imgs.length === 0) throw new Error("没有可预览帧");
        if (state.previewTimer) clearInterval(state.previewTimer);
        zoom = 1;
        previewArea.innerHTML = `<img id="animFrame" style="max-width:100%;max-height:100%;width:auto;height:auto;transform-origin:center;border:1px solid var(--border)">`;
        const img = $("#animFrame");
        let idx = 0;
        img.src = imgs[0];
        $("#frameCounter").textContent = `1 / ${imgs.length}`;
        renderFrameDetail(0);
        const fps = Math.max(1, data.frame_rate || 15);
        state.previewTimer = setInterval(() => {
          idx = (idx + 1) % imgs.length;
          img.src = imgs[idx];
          $("#frameCounter").textContent = `${idx + 1} / ${imgs.length}`;
          renderFrameDetail(idx);
        }, 1000 / fps);
      } catch (e) {
        previewArea.innerHTML = `<p style="color:var(--err)">失败：${esc(e.message)}</p>`;
      }
    };
    startPreview();

    let refreshTimer = null;
    const schedulePreviewRefresh = () => {
      if (refreshTimer) clearTimeout(refreshTimer);
      refreshTimer = setTimeout(() => startPreview(), 250);
    };

    $("#exportGif").onclick = async () => {
      const res = await fetch(`/api/anim/assets/render?${buildRenderParams("gif")}`);
      if (!res.ok) { alert("导出失败：" + await res.text()); return; }
      const blob = await res.blob();
      const a = document.createElement("a");
      a.href = URL.createObjectURL(blob);
      a.download = `${state.selectedFile.replace(/\W+/g, "_")}-${state.selectedAnim}.gif`;
      a.click();
    };
    $("#exportPng").onclick = async () => {
      const res = await fetch(`/api/anim/assets/render?${buildRenderParams("png")}`);
      if (!res.ok) { alert("导出失败：" + await res.text()); return; }
      const blob = await res.blob();
      const a = document.createElement("a");
      a.href = URL.createObjectURL(blob);
      a.download = `${state.selectedFile.replace(/\W+/g, "_")}-${state.selectedAnim}-frames.zip`;
      a.click();
    };
  }


  // Initial load
  main.innerHTML = '<div class="panel"><p class="muted">加载中…</p></div>';
  await search();
}

/* ---------------- animation diff ---------------- */
async function pageAnims(main) {
  main.innerHTML = `
    <div class="panel">
      <h2>动画历史对比</h2>
      <div class="row">
        <label style="margin:0">从</label><select id="animFrom"></select>
        <label style="margin:0">到</label><select id="animTo"></select>
        <input id="animZip" placeholder="可选：精确到文件，如 dynamic/a.dyn" style="width:260px">
        <button class="btn" id="runAnimDiff">开始对比</button>
      </div>
      <p class="muted">对比两份 anim-sync 历史 manifest（通过 CAS 还原原始 zip/dyn 后解析）。</p>
    </div>
    <div id="animDiffOut"></div>`;

  const fromSel = $("#animFrom"), toSel = $("#animTo");
  const manifests = (await getJSON("/api/anim/manifests")).manifests || [];
  const mkOption = (v) => `<option value="${esc(v)}">${esc(v)}</option>`;
  fromSel.innerHTML = manifests.map(mkOption).join("");
  toSel.innerHTML = manifests.map(mkOption).join("");
  if (manifests.length >= 1) toSel.selectedIndex = manifests.length - 1;
  if (manifests.length > 1) fromSel.selectedIndex = manifests.length - 2;

  $("#runAnimDiff").onclick = async () => {
    const from = fromSel.value, to = toSel.value;
    if (!from || !to) { alert("请先选择两个历史版本"); return; }
    const zip = $("#animZip").value.trim();
    const url = `/api/anim/diff?from=${encodeURIComponent(from)}&to=${encodeURIComponent(to)}` +
      (zip ? `&zip=${encodeURIComponent(zip)}` : "");
    $("#animDiffOut").innerHTML = '<p class="muted">对比中…</p>';
    try {
      const d = await getJSON(url);
      renderAnimDiff(d);
    } catch (e) {
      $("#animDiffOut").innerHTML = `<div class="panel" style="color:var(--err)">失败：${esc(e.message)}</div>`;
    }
  };

  function renderAnimDiff(d) {
    const fd = d.file_diff || {};
    const files = Object.entries(d.archives || {});
    const changed = (fd.changed || []).length;
    const added = (fd.added || []).length;
    const removed = (fd.removed || []).length;
    const totalChanges = added + changed + removed;
    $("#animDiffOut").innerHTML = `
      <div class="card-row">
        <div class="card"><div class="num diff-added">${added}</div><span class="muted">新增文件</span></div>
        <div class="card"><div class="num diff-removed">${removed}</div><span class="muted">移除文件</span></div>
        <div class="card"><div class="num diff-changed">${changed}</div><span class="muted">修改文件</span></div>
        <div class="card"><div class="num">${totalChanges}</div><span class="muted">总变更</span></div>
      </div>
      <div class="panel">
        <h2>文件级变更</h2>
        ${fd.added && fd.added.length ? `<div><span class="muted">新增：</span>${fd.added.map(a => `<span class="chip diff-added">${esc(a)}</span>`).join("")}</div>` : ""}
        ${fd.removed && fd.removed.length ? `<div><span class="muted">移除：</span>${fd.removed.map(a => `<span class="chip diff-removed">${esc(a)}</span>`).join("")}</div>` : ""}
        ${fd.changed && fd.changed.length ? `<div><span class="muted">修改：</span>${fd.changed.map(a => `<span class="chip diff-changed">${esc(a.path)}</span>`).join("")}</div>` : ""}
        ${totalChanges === 0 ? '<p class="muted">两个版本没有文件级变化。</p>' : ""}
      </div>
      <div class="panel">
        <h2>结构化 diff</h2>
        ${files.length === 0 ? '<p class="muted">无结构化变更。</p>' : `
        <table><thead><tr><th>文件</th><th>状态</th><th>Anim</th><th>Build</th><th>Tex</th><th></th></tr></thead><tbody>
        ${files.map(([path, v]) => {
          const animChanged = v.anim ? (v.anim.changed ? v.anim.details.length : 0) : 0;
          const buildChanged = v.build ? (v.build.changed ? v.build.details.length : 0) : 0;
          const texKeys = Object.keys(v.tex_changed || {}).length;
          const statusCls = v.status === "added" ? "b-ok" : v.status === "removed" ? "b-err" : v.status === "modified" ? "b-warn" : "";
          const details = [];
          if (v.anim && v.anim.details && v.anim.details.length) {
            details.push(`<h4>anim</h4><ul>${v.anim.details.map(c => `<li><code>${esc(c.path)}</code>: ${esc(JSON.stringify(c.old))} → <b>${esc(JSON.stringify(c.new))}</b></li>`).join("")}</ul>`);
          }
          if (v.build && v.build.details && v.build.details.length) {
            details.push(`<h4>build</h4><ul>${v.build.details.map(c => `<li><code>${esc(c.path)}</code>: ${esc(JSON.stringify(c.old))} → <b>${esc(JSON.stringify(c.new))}</b></li>`).join("")}</ul>`);
          }
          if (texKeys) {
            details.push(`<h4>tex</h4><ul>${Object.entries(v.tex_changed).map(([k, val]) => `<li><code>${esc(k)}</code>: ${esc(val)}</li>`).join("")}</ul>`);
          }
          if (v.parse_error) {
            details.push(`<p style="color:var(--err)">${esc(v.parse_error)}</p>`);
          }
          return `<tr>
            <td><code>${esc(path)}</code></td>
            <td><span class="badge ${statusCls}">${esc(v.status)}</span></td>
            <td>${animChanged || "—"}</td>
            <td>${buildChanged || "—"}</td>
            <td>${texKeys || "—"}</td>
            <td>${details.length ? `<details><summary>明细</summary>${details.join("")}</details>` : ""}</td>
          </tr>`;
        }).join("")}
        </tbody></table>`}
      </div>`;
  }
}

/* ---------------- snapshot diff ---------------- */
async function pageSnapshots(main) {
  main.innerHTML = `
    <div class="panel">
      <h2>游戏快照对比</h2>
      <div class="row">
        <label style="margin:0">从</label><select id="fromSel">${snapshotOptions()}</select>
        <label style="margin:0">到</label><select id="toSel">${snapshotOptions()}</select>
        <select id="diffKind"><option value="recipes">配方</option><option value="po">翻译</option><option value="remaps">重映射</option></select>
        <button class="btn" id="runDiff">开始对比</button>
      </div>
      <p class="muted">对比两个 scripts 快照目录之间的差异。首次对比需要解析两份数据，可能需要数秒。</p>
    </div>
    <div id="diffOut"></div>`;

  function snapshotOptions() {
    // filled asynchronously below
    return "";
  }

  const fromSel = $("#fromSel"), toSel = $("#toSel");
  const loadOptions = async (kind) => {
    let names;
    if (kind === "remaps") {
      names = ((await getJSON("/api/anim/remap-manifests")).manifests || []).map(m => m.label);
    } else {
      // recipes/po 对比的是 scripts 快照目录
      names = (await getJSON("/api/snapshots")).snapshots.map(s => s.name);
    }
    const mkOption = (v) => `<option value="${esc(v)}">${esc(v)}</option>`;
    fromSel.innerHTML = names.map(mkOption).join("");
    toSel.innerHTML = names.map(mkOption).join("");
    if (names.length >= 1) toSel.selectedIndex = 0;
    if (names.length > 1) fromSel.selectedIndex = 1;
  };
  await loadOptions("recipes");
  $("#diffKind").onchange = () => loadOptions($("#diffKind").value).catch(() => {});

  $("#runDiff").onclick = async () => {
    const kind = $("#diffKind").value;
    const url = `/api/diff/${kind}?from=${fromSel.value}&to=${toSel.value}`;
    $("#diffOut").innerHTML = '<p class="muted">对比中…</p>';
    try {
      const d = await getJSON(url);
      if (kind === "recipes") renderRecipesDiff(d);
      else if (kind === "remaps") renderRemapDiff(d);
      else renderPoDiff(d);
    } catch (e) { $("#diffOut").innerHTML = `<div class="panel" style="color:var(--err)">失败：${esc(e.message)}</div>`; }
  };

  function renderRemapDiff(d) {
    const chip = (cls, text) => `<span class="chip ${cls}">${esc(text)}</span>`;
    const entryLabel = (e) => `${e.symbol} ← ${e.build}/${e.src_symbol} (${e.api})`;
    const key = (e) => `${e.symbol}|${e.build}|${e.src_symbol}|${e.api}`;
    $("#diffOut").innerHTML = `
      ${d.parser_version_changed ? '<div class="panel" style="border-color:var(--warn)"><span class="badge b-warn">解析器版本不同</span><span class="muted"> 本 diff 混有解析器升级带来的差异（如 static ↔ resolved），结论需人工甄别。</span></div>' : ""}
      <div class="card-row">
        <div class="card"><div class="num diff-added">${d.symbols_added.length}</div><span class="muted">新增 symbol</span></div>
        <div class="card"><div class="num diff-removed">${d.symbols_removed.length}</div><span class="muted">移除 symbol</span></div>
        <div class="card"><div class="num diff-added">${d.entries_added.length}</div><span class="muted">新增条目</span></div>
        <div class="card"><div class="num diff-removed">${d.entries_removed.length}</div><span class="muted">移除条目</span></div>
        <div class="card"><div class="num diff-changed">${d.prefabs_changed.length + d.confidence_changed.length + d.clothing_changed.length}</div><span class="muted">其他变更</span></div>
      </div>
      <div class="panel">
        <h2>条目级变更</h2>
        ${d.entries_added.length ? `<div style="margin:4px 0">${d.entries_added.map(e => chip("diff-added", `+ ${entryLabel(e)}`)).join(" ")}</div>` : ""}
        ${d.entries_removed.length ? `<div style="margin:4px 0">${d.entries_removed.map(e => chip("diff-removed", `- ${entryLabel(e)}`)).join(" ")}</div>` : ""}
        ${(d.entries_added.length + d.entries_removed.length) === 0 ? '<p class="muted">无条目级变更。</p>' : ""}
      </div>
      ${d.prefabs_changed.length ? `
      <div class="panel">
        <h2>来源（prefab）变更</h2>
        <table><thead><tr><th>条目</th><th>新增来源</th><th>移除来源</th></tr></thead><tbody>
        ${d.prefabs_changed.map(c => `<tr><td><code>${esc(entryLabel(c.entry))}</code></td>
          <td>${c.added.map(a => chip("diff-added", a)).join(" ") || "—"}</td>
          <td>${c.removed.map(a => chip("diff-removed", a)).join(" ") || "—"}</td></tr>`).join("")}
        </tbody></table></div>` : ""}
      ${d.confidence_changed.length ? `
      <div class="panel">
        <h2>置信度变更</h2>
        <table><thead><tr><th>条目</th><th>变化</th></tr></thead><tbody>
        ${d.confidence_changed.map(c => `<tr><td><code>${esc(entryLabel(c.entry))}</code></td>
          <td><span class="chip">${esc(c.old)}</span> → <span class="chip ${c.new === "resolved" ? "diff-changed" : "diff-added"}">${esc(c.new)}</span></td></tr>`).join("")}
        </tbody></table></div>` : ""}
      <div class="panel">
        <h2>Clothing 数据表</h2>
        <div style="margin:4px 0">
          ${d.clothing_added.map(n => chip("diff-added", `+ ${n}`)).join(" ")}
          ${d.clothing_removed.map(n => chip("diff-removed", `- ${n}`)).join(" ")}
          ${d.clothing_changed.map(c => chip("diff-changed", `~ ${c.name}`)).join(" ")}
          ${(d.clothing_added.length + d.clothing_removed.length + d.clothing_changed.length) === 0 ? '<p class="muted">无变更。</p>' : ""}
        </div>
        ${d.clothing_changed.length ? `<details><summary>${d.clothing_changed.length} 条明细</summary>
          <table><thead><tr><th>名称</th><th>变更前后</th></tr></thead><tbody>
          ${d.clothing_changed.map(c => `<tr><td><code>${esc(c.name)}</code></td><td><details><summary>查看</summary><div style="display:flex;gap:12px"><pre style="max-width:420px;overflow:auto">${esc(JSON.stringify(c.old, null, 1))}</pre><pre style="max-width:420px;overflow:auto">${esc(JSON.stringify(c.new, null, 1))}</pre></div></details></td></tr>`).join("")}
          </tbody></table></details>` : ""}
      </div>
      <div class="panel muted">总量：symbol ${d.totals.from_symbols} → ${d.totals.to_symbols}，条目 ${d.totals.from_entries} → ${d.totals.to_entries}，clothing ${d.totals.from_clothing} → ${d.totals.to_clothing}</div>`;
  }

  function renderRecipesDiff(d) {
    $("#diffOut").innerHTML = `
      <div class="card-row">
        <div class="card"><div class="num diff-added">${d.added.length}</div><span class="muted">新增配方</span></div>
        <div class="card"><div class="num diff-removed">${d.removed.length}</div><span class="muted">移除配方</span></div>
        <div class="card"><div class="num diff-changed">${d.changed.length}</div><span class="muted">修改配方</span></div>
        <div class="card"><div class="num">${d.total_from} → ${d.total_to}</div><span class="muted">总量变化</span></div>
      </div>
      <div class="grid2">
        <div class="panel"><h2>新增</h2><div>${d.added.map(a => `<span class="chip diff-added">${esc(a)}</span>`).join("") || '<span class="muted">无</span>'}</div></div>
        <div class="panel"><h2>移除</h2><div>${d.removed.map(a => `<span class="chip diff-removed">${esc(a)}</span>`).join("") || '<span class="muted">无</span>'}</div></div>
      </div>
      <div class="panel"><h2>修改明细</h2>
        <table><thead><tr><th>名称</th><th>科技</th><th>材料变化</th></tr></thead><tbody>
        ${d.changed.map(c => `<tr><td><code>${esc(c.name)}</code></td>
          <td>${c.tech_before ? `${esc(c.tech_before)} → <b class="diff-changed">${esc(c.tech_after)}</b>` : "—"}</td>
          <td>${[
            ...c.amounts_changed.map(a => `${esc(a.item)} ${a.before}→<b>${a.after}</b>`),
            ...c.ingredients_added.map(a => `<span class="diff-added">+${esc(a.item)}×${a.amount}</span>`),
            ...c.ingredients_removed.map(a => `<span class="diff-removed">-${esc(a.item)}×${a.amount}</span>`),
          ].join("、") || "—"}</td></tr>`).join("") || '<tr><td colspan="3" class="muted">无修改</td></tr>'}
        </tbody></table></div>`;
  }

  function renderPoDiff(d) {
    $("#diffOut").innerHTML = `
      <div class="card-row">
        <div class="card"><div class="num diff-added">${d.keys_added}</div><span class="muted">新增键</span></div>
        <div class="card"><div class="num diff-removed">${d.keys_removed}</div><span class="muted">移除键</span></div>
        <div class="card"><div class="num diff-changed">${d.translations_changed}</div><span class="muted">译文变化</span></div>
        <div class="card"><div class="num">${d.newly_translated}</div><span class="muted">新增翻译完成</span></div>
      </div>
      <div class="grid2">
        <div class="panel"><h2>新增样例（前 50）</h2><div>${d.added_samples.map(s => `<span class="chip">${esc(s)}</span>`).join("") || '<span class="muted">无</span>'}</div></div>
        <div class="panel"><h2>移除样例（前 50）</h2><div>${d.removed_samples.map(s => `<span class="chip">${esc(s)}</span>`).join("") || '<span class="muted">无</span>'}</div></div>
      </div>
      <div class="panel muted">条目总量：${d.total_from} → ${d.total_to}</div>`;
  }
}

/* ---------------- boot ---------------- */
window.addEventListener("hashchange", navigate);
navigate();
