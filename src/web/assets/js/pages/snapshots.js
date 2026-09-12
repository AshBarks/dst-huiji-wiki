//! 快照对比页
"use strict";

import { esc, api, getJSON, $ } from "../util.js";

export async function pageSnapshots(main) {
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
