//! 动画对比页
"use strict";

import { esc, api, getJSON, $ } from "../util.js";

export async function pageAnims(main) {
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
