//! 概览页
"use strict";

import { esc, fmtTime, api, getJSON, postJSON, statusBadge, META, currentSnapshot, $ } from "../util.js";

export async function pageDashboard(main) {
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
