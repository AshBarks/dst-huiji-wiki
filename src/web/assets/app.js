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

  main.innerHTML = `
    <div class="card-row">
      <div class="card"><div class="num">${META.recipe_count}</div><span class="muted">配方</span></div>
      <div class="card"><div class="num">${META.ingredient_count}</div><span class="muted">材料种类</span></div>
      <div class="card"><div class="num">${META.po_total_entries}</div><span class="muted">翻译条目</span></div>
      <div class="card"><div class="num">${META.tuning_count}</div><span class="muted">TUNING 常量</span></div>
      <div class="card"><div class="num">${snaps}</div><span class="muted">历史快照</span></div>
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
}

/* ---------------- jobs ---------------- */
const JOB_FIELDS = {
  "parse_po": [["input", "PO 文件路径"], ["category", "类别过滤（可选）"]],
  "map_names": [["input", "PO 文件路径"], ["version", "版本号（可选）"], ["compare", "对比文件（可选）"]],
  "map_recipes": [["input", "recipes.lua 路径"], ["po_file", "PO 文件（可选）"], ["version", "版本号（可选）"], ["compare", "对比文件（可选）"]],
  "maintain_item_table": [],
  "maintain_dst_recipes": [],
  "maintain_copyclip": [["type", "类型：rbtl / tech / filters / names（留空=全部）"]],
  "prefab_overrides": [["input", "Lua 文件路径"]],
};
const JOB_LABELS = {
  parse_po: "parse-po 解析 PO", map_names: "map-names 名称映射", map_recipes: "map-recipes 配方映射",
  maintain_item_table: "维护物品表 → 维基", maintain_dst_recipes: "维护配方表 → 维基",
  maintain_copyclip: "维护模块常量 → 维基", prefab_overrides: "预制体重定向解析",
};

async function pageJobs(main) {
  main.innerHTML = `
    <div class="panel">
      <h2>提交新任务</h2>
      <div class="row">
        <select id="jobKind">${Object.entries(JOB_LABELS)
          .map(([k, v]) => `<option value="${k}">${v}</option>`).join("")}</select>
        <label style="margin:0;display:flex;align-items:center;gap:6px;color:var(--text)">
          <input type="checkbox" id="dryRun" checked> 干跑模式（不写入维基）</label>
        <button class="btn" id="submitJob">提交任务</button>
      </div>
      <div id="jobFields"></div>
      <p class="muted">涉及维基写入的任务在干跑模式下只会生成 diff 预览；取消勾选后将在页面确认过参数的前提下直接执行写入。</p>
    </div>
    <div class="panel">
      <h2>任务列表</h2>
      <div id="jobList"></div>
    </div>`;

  const kindSel = $("#jobKind");
  const renderFields = () => {
    const fields = JOB_FIELDS[kindSel.value] || [];
    $("#jobFields").innerHTML = fields
      .map(([k, label]) => `<label>${esc(label)}<input style="width:100%" data-field="${k}"></label>`)
      .join("");
  };
  kindSel.onchange = renderFields;
  renderFields();

  $("#submitJob").onclick = async () => {
    const body = { kind: kindSel.value, dry_run: $("#dryRun").checked };
    document.querySelectorAll("#jobFields input[data-field]").forEach(i => {
      const v = i.value.trim();
      if (v !== "") body[i.dataset.field] = v;
    });
    if (!body.dry_run && !confirm("已关闭干跑模式：任务可能直接修改维基页面。确定继续？")) return;
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
          <td>${statusBadge(j.status)}</td><td>${j.touches_wiki && j.dry_run ? '<span class="badge b-warn">干跑</span> ' : ""}<code>${esc(j.kind)}</code></td>
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
    const canApprove = h.touches_wiki && h.dry_run && h.status === "success";
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
      if (!confirm("将使用相同参数真实写入维基页面（不再走干跑）。确定继续？")) return;
      ab.disabled = true;
      try {
        const j = await postJSON("/api/jobs", Object.assign({}, h.params, { dry_run: false }));
        location.hash = `#/jobs/${j.id}`;
      } catch (e) { ab.disabled = false; alert("提交失败：" + e.message); }
    };
  };

  const head = await getJSON(`/api/jobs/${id}`);
  renderHead(head);
  for (const ev of head.logs || []) appendEv(ev);
  for (const d of head.diffs || []) appendDiff(d);

  if (!doneSeen && !["success", "failed", "cancelled"].includes(head.status)) {
    const es = new EventSource(`/api/jobs/${id}/events`);
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
    <div class="panel" id="tdetail" style="display:none">
      <h2 id="ttitle"></h2>
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
    $("#tdetail").style.display = "";
    $("#ttitle").textContent = `分类：${st.cat}`;
    await loadEntries();
    $("#tdetail").scrollIntoView({ behavior: "smooth" });
  });
  let deb;
  $("#tq").oninput = (e) => { clearTimeout(deb); deb = setTimeout(() => { st.q = e.target.value.trim(); st.page = 0; loadEntries(); }, 250); };
  $("#ttf").onchange = (e) => { st.tf = e.target.value; st.page = 0; loadEntries(); };
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
      </div>
    </div>
    <div class="panel"><div id="skwrap"></div></div>
    <div id="tooltip"></div>`;

  const sel = $("#charSel");
  sel.value = chars.includes("wilson") ? "wilson" : chars[0];
  const draw = async () => {
    const tree = await getJSON(`/api/viz/skilltree?character=${sel.value}`);
    const nodes = tree.nodes;
    if (!nodes.length) { $("#skwrap").innerHTML = '<p class="muted">该角色暂无技能树数据。</p>'; return; }
    const xs = nodes.flatMap(n => [n.x]); const ys = nodes.flatMap(n => [n.y]);
    const pad = 40, minX = Math.min(...xs) - pad, maxX = Math.max(...xs) + pad;
    const minY = Math.min(...ys) - pad, maxY = Math.max(...ys) + pad;
    const byName = Object.fromEntries(nodes.map(n => [n.name, n]));
    const palette = ["#4da3ff","#3fb96e","#e0a83c","#9a7de0","#e05c5c","#4ec9d4","#d4874e","#7dc46a"];
    const groupColor = {}; tree.groups.forEach((g, i) => groupColor[g] = palette[i % palette.length]);

    const edges = [];
    for (const n of nodes) for (const c of n.connects) {
      const m = byName[c]; if (!m) continue;
      edges.push(`<line class="edge" x1="${n.x}" y1="${-n.y}" x2="${m.x}" y2="${-m.y}"/>`);
    }
    const dots = nodes.map(n => {
      const color = n.group ? groupColor[n.group] : "#888";
      const title = n.title || n.name;
      return `<g class="node" data-name="${esc(n.name)}"
        data-title="${esc(title)}" data-desc="${esc(n.desc || "")}"
        data-group="${esc(n.group || "")}">
        <circle cx="${n.x}" cy="${-n.y}" r="${n.root ? 13 : 9}"
          fill="${color}${n.root ? "" : "55"}" stroke="${color}"/>
        <text x="${n.x}" y="${-n.y + (n.root ? 28 : 24)}">${esc(String(title).slice(0, 12))}</text>
      </g>`;
    }).join("");

    $("#skwrap").innerHTML =
      `<svg viewBox="${minX} ${-maxY} ${maxX - minX} ${maxY - minY}" width="100%" style="background:#151923;border-radius:10px">
        ${edges.join("")}${dots}</svg>`;

    const tip = $("#tooltip");
    document.querySelectorAll("#skwrap .node").forEach(nd => {
      nd.addEventListener("mousemove", (e) => {
        tip.style.display = "block";
        tip.style.left = (e.clientX + 14) + "px"; tip.style.top = (e.clientY + 14) + "px";
        tip.innerHTML = `<b>${esc(nd.dataset.title)}</b><br>
          ${nd.dataset.desc ? esc(nd.dataset.desc) + "<br>" : ""}
          <span class="muted"><code>${esc(nd.dataset.name)}</code>${nd.dataset.group ? " · " + esc(nd.dataset.group) : ""}</span>`;
      });
      nd.addEventListener("mouseleave", () => tip.style.display = "none");
    });
  };
  sel.onchange = draw;
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
