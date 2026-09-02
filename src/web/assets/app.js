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
  ["constants", "常量", pageConstants],
  ["snapshots", "快照对比", pageSnapshots],
];

function navigate() { render(location.hash.replace(/^#\/?/, "") || ""); }

async function render(path) {
  if (routeTimer) { clearInterval(routeTimer); routeTimer = null; }
  const [head] = path.split("/");
  $("#nav").innerHTML = routes.map(([seg, label]) =>
    `<a href="#/${seg}" class="${seg === head ? "active" : ""}">${label}</a>`).join("");

  const main = $("#app");
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
        <span class="muted">布局坐标来自游戏源码（pos/connects），1:1 还原游戏内界面。</span>
        <span class="muted" id="skillXp"></span>
        <button class="btn secondary" id="resetSkillBtn">重置洞察</button>
      </div>
      <p class="muted" style="margin:6px 0 0">点击“可选”技能学习；lock 节点按条件自动解锁；重置洞察会清空已学技能。</p>
    </div>
    <div class="panel"><div id="skwrap"></div></div>
    <div id="tooltip"></div>`;

  const sel = $("#charSel");
  sel.value = chars.includes("wilson") ? "wilson" : chars[0];

  const TOTAL_XP = 15;
  let tree = null;
  let skills = {};
  let locks = {};
  let parents = {};
  let activatedSkills = new Set();

  const iconUrl = (icon) => icon ? `/static/split/skilltree_icons/${encodeURIComponent(icon)}.png` : "";
  const skillAsset = (name) => `/static/split/skilltree/${encodeURIComponent(name)}.png`;
  const ICON_SIZE = 28;
  const ICON_BUTTON_SIZE = 32;
  const LOCK_SIZE = ICON_SIZE * 0.8; // wiki JS: lock button = 28 * 0.8
  const FOCUS_SIZE = 40;

  function buildMaps(nodes) {
    skills = {};
    locks = {};
    parents = {};
    for (const n of nodes) {
      if (n.lock) {
        locks[n.name] = n;
      } else {
        skills[n.name] = n;
        parents[n.name] = [];
      }
    }
    for (const n of nodes) {
      if (n.lock) continue;
      for (const c of (n.connects || [])) {
        if (parents[c]) parents[c].push(n.name);
      }
    }
  }

  function remainingXp() {
    return TOTAL_XP - activatedSkills.size;
  }

  function countTags(tag) {
    let count = 0;
    for (const name of activatedSkills) {
      const skill = skills[name];
      if (skill && skill.tags && skill.tags.includes(tag)) {
        count += 1;
      }
    }
    return count;
  }

  function evalLockCond(cond) {
    if (cond === true || cond === false || typeof cond === "number" || typeof cond === "string") {
      return cond;
    }
    if (typeof cond !== "object" || cond === null) {
      return false;
    }
    if (cond.Achievement) {
      // 外部成就类条件在本地默认视为已解锁。
      return true;
    }
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
    // 未显式给出条件时默认视为已解锁，避免外部成就/未知条件把整条线路锁死。
    if (!lock || lock.lock_open === undefined || lock.lock_open === null) return true;
    return evalLockCond(lock.lock_open);
  }

  function canLearn(name) {
    const skill = skills[name];
    if (!skill || activatedSkills.has(name)) return false;
    if (remainingXp() <= 0) return false;
    if (skill.root) return true;
    if (skill.locks && skill.locks.some(l => !isLockOpen(l))) return false;
    const ps = parents[name] || [];
    if (ps.length > 0 && !ps.some(p => activatedSkills.has(p))) return false;
    return true;
  }

  function statusOf(name) {
    if (activatedSkills.has(name)) return "selected";
    return canLearn(name) ? "selectable" : "unselected";
  }

  function render() {
    if (!tree) return;
    const nodes = tree.nodes;
    if (!nodes.length) {
      $("#skwrap").innerHTML = '<p class="muted">该角色暂无技能树数据。</p>';
      return;
    }
    const WIDTH = 600;
    const HEIGHT = 540;
    const SVG_HEIGHT = 460;
    const X_SCALE = 1;
    const X_OFFSET = -2;
    const Y_SCALE = 1.157;
    const Y_OFFSET = 50 + 30 - 20;
    const yScale = ["wendy", "wortox"].includes(sel.value.toLowerCase()) ? 1 : Y_SCALE;
    const px = (x) => WIDTH / 2 + X_SCALE * (X_OFFSET + x);
    const py = (y) => HEIGHT / 2 - yScale * (y - Y_OFFSET);
    const byName = Object.fromEntries(nodes.map(n => [n.name, n]));

    const edges = [];
    for (const n of nodes) for (const c of (n.connects || [])) {
      const m = byName[c];
      if (!m) continue;
      edges.push(`<line class="edge" x1="${px(n.x)}" y1="${py(n.y)}" x2="${px(m.x)}" y2="${py(m.y)}"/>`);
    }

    const charBg = skillAsset(`${sel.value}_background`);
    const genericBg = skillAsset("background");

    const dots = nodes.map(n => {
      const title = n.title || n.name;
      const x = px(n.x);
      const y = py(n.y);
      const size = n.lock ? LOCK_SIZE : ICON_BUTTON_SIZE;
      const bgName = n.lock
        ? (isLockOpen(n.name) ? "unlocked" : "locked_skill")
        : statusOf(n.name);
      const focusName = n.lock ? "frame_octagon" : "frame";
      const glyph = `<image href="${esc(skillAsset(bgName))}" x="${x - size / 2}" y="${y - size / 2}" width="${size}" height="${size}" preserveAspectRatio="xMidYMid meet"/>` +
        (n.lock || !n.icon ? "" : `<image href="${esc(iconUrl(n.icon))}" x="${x - ICON_SIZE / 2}" y="${y - ICON_SIZE / 2}" width="${ICON_SIZE}" height="${ICON_SIZE}" preserveAspectRatio="xMidYMid meet"/>`) +
        `<image class="node-focus" href="${esc(skillAsset(focusName))}" x="${x - FOCUS_SIZE / 2}" y="${y - FOCUS_SIZE / 2}" width="${FOCUS_SIZE}" height="${FOCUS_SIZE}" style="display:none" preserveAspectRatio="xMidYMid meet"/>`;
      return `<g class="node" data-name="${esc(n.name)}"
        data-title="${esc(title)}" data-desc="${esc(n.desc || "")}"
        data-group="${esc(n.group || "")}" data-icon="${esc(n.icon || "")}" data-lock="${n.lock}">
        ${glyph}
        <text x="${x}" y="${y + (n.root ? 28 : 24)}">${esc(String(title).slice(0, 12))}</text>
      </g>`;
    }).join("");

    $("#skwrap").innerHTML =
      `<svg viewBox="0 0 ${WIDTH} ${SVG_HEIGHT}" width="100%" style="display:block;width:100%;height:auto;aspect-ratio:${WIDTH} / ${SVG_HEIGHT};background:#151923;border-radius:10px">
        <image href="${esc(genericBg)}" x="0" y="0" width="${WIDTH}" height="${HEIGHT}" preserveAspectRatio="xMidYMid meet"/>
        <image href="${esc(charBg)}" x="0" y="0" width="${WIDTH}" height="${HEIGHT}" preserveAspectRatio="xMidYMid meet"/>
        ${edges.join("")}${dots}</svg>`;

    $("#skillXp").textContent = `剩余洞察：${remainingXp()}`;

    const tip = $("#tooltip");
    document.querySelectorAll("#skwrap .node").forEach(nd => {
      const focusEle = nd.querySelector(".node-focus");
      const name = nd.dataset.name;
      const isLock = nd.dataset.lock === "true";

      nd.addEventListener("click", () => {
        if (!isLock && canLearn(name)) {
          activatedSkills.add(name);
          render();
        }
      });

      nd.addEventListener("mousemove", (e) => {
        if (focusEle) focusEle.style.display = "inline";
        tip.style.display = "block";
        tip.style.left = (e.clientX + 14) + "px";
        tip.style.top = (e.clientY + 14) + "px";
        const tooltipIcon = nd.dataset.icon
          ? iconUrl(nd.dataset.icon)
          : isLock ? skillAsset(isLockOpen(name) ? "unlocked" : "locked_skill") : "";
        const iconHtml = tooltipIcon
          ? `<img src="${esc(tooltipIcon)}" style="width:44px;height:44px;float:left;margin-right:8px;border-radius:6px">`
          : "";
        tip.innerHTML = `<div style="overflow:hidden">${iconHtml}<b>${esc(nd.dataset.title)}</b><br>
          ${nd.dataset.desc ? esc(nd.dataset.desc) + "<br>" : ""}
          ${isLock ? `<span class="muted">${isLockOpen(name) ? "已解锁" : "未解锁"}</span><br>` : ""}
          <span class="muted"><code>${esc(nd.dataset.name)}</code>${nd.dataset.group ? " · " + esc(nd.dataset.group) : ""}</span></div>`;
      });

      nd.addEventListener("mouseleave", () => {
        if (focusEle) focusEle.style.display = "none";
        tip.style.display = "none";
      });
    });
  }

  async function draw() {
    tree = await getJSON(`/api/viz/skilltree?character=${sel.value}`);
    buildMaps(tree.nodes);
    activatedSkills.clear();
    render();
  }

  sel.onchange = () => { draw(); };
  $("#resetSkillBtn").onclick = () => {
    activatedSkills.clear();
    render();
  };

  await draw();
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

/* ---------------- snapshot diff ---------------- */
async function pageSnapshots(main) {
  main.innerHTML = `
    <div class="panel">
      <h2>游戏快照对比</h2>
      <div class="row">
        <label style="margin:0">从</label><select id="fromSel">${snapshotOptions()}</select>
        <label style="margin:0">到</label><select id="toSel">${snapshotOptions()}</select>
        <select id="diffKind"><option value="recipes">配方</option><option value="po">翻译</option></select>
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
  const snaps = (await getJSON("/api/snapshots")).snapshots.map(s => s.name);
  const mkOption = (v) => `<option value="${esc(v)}">${esc(v)}</option>`;
  fromSel.innerHTML = snaps.map(mkOption).join("");
  toSel.innerHTML = snaps.map(mkOption).join("");
  if (snaps.length >= 1) toSel.selectedIndex = 0;
  if (snaps.length > 1) fromSel.selectedIndex = 1;

  $("#runDiff").onclick = async () => {
    const kind = $("#diffKind").value;
    const url = `/api/diff/${kind}?from=${fromSel.value}&to=${toSel.value}`;
    $("#diffOut").innerHTML = '<p class="muted">对比中…</p>';
    try {
      const d = await getJSON(url);
      if (kind === "recipes") renderRecipesDiff(d); else renderPoDiff(d);
    } catch (e) { $("#diffOut").innerHTML = `<div class="panel" style="color:var(--err)">失败：${esc(e.message)}</div>`; }
  };

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
