//! 概览页
"use strict";

import { esc, fmtTime, getJSON, postJSON, statusBadge, META, currentSnapshot, $ } from "../util.js";

export async function pageDashboard(main) {
  let jobs = [];
  try { jobs = (await getJSON("/api/jobs")).jobs.slice(0, 6); } catch {}
  const snaps = (await getJSON("/api/snapshots")).snapshots;
  const cfg = await getJSON("/api/config");
  const ss = cfg.scripts_sync || {};
  const scriptsStatus = !ss.ready
    ? "无法读取 version.txt，请检查 DST__ROOT"
    : ss.up_to_date
      ? `已是最新版本：${esc(ss.detected_version)}`
      : `记录 ${esc(ss.recorded_version || "无")} → 检测 ${esc(ss.detected_version)}，可同步`;

  const snapOptions = [`<option value="">最新 / live 树</option>`]
    .concat(snaps.map(s => `<option value="${esc(s.name)}">${esc(s.name)}</option>`))
    .join("");

  main.innerHTML = `
    <div class="card-row">
      <div class="card"><div class="num">${META.recipe_count}</div><span class="muted">配方</span></div>
      <div class="card"><div class="num">${META.ingredient_count}</div><span class="muted">材料种类</span></div>
      <div class="card"><div class="num">${META.po_total_entries}</div><span class="muted">翻译条目</span></div>
      <div class="card"><div class="num">${META.tuning_count}</div><span class="muted">TUNING 常量</span></div>
      <div class="card"><div class="num">${snaps.length}</div><span class="muted">历史快照</span></div>
    </div>
    <div class="panel">
      <h2>版本更新</h2>
      <p class="muted" style="margin-top:0">${scriptsStatus}</p>
      <div class="card-row">
        <div class="card quick-card">
          <div><b>update-local</b> <span class="muted">本地摄取</span></div>
          <div class="steps">
            <span class="step">scripts-sync</span><span class="step">images-sync</span><span class="step">anim-sync</span>
          </div>
          <p class="muted" style="margin:0">按序归档旧脚本树、解码游戏图片、归档动画历史；任一步失败即中止。</p>
          <label style="display:flex;align-items:center;gap:6px;margin:0;color:var(--text)">
            <input type="checkbox" id="ulDry" checked> 演练（不写任何本地文件）
          </label>
          <div><button class="btn" id="ulRun">开始本地摄取</button></div>
        </div>
        <div class="card quick-card">
          <div><b>update-data</b> <span class="muted">数据页 → 维基</span></div>
          <div class="steps">
            <span class="step">copy-clip</span><span class="step">dst-recipes</span>
            <span class="step">item-table</span><span class="step">strings</span>
          </div>
          <p class="muted" style="margin:0">先写模块常量（科技/制作站），再同步配方与物品表，最后只写变化的 Strings 桶页。</p>
          <label>scripts 快照
            <select id="udSnap">${snapOptions}</select>
          </label>
          <label style="display:flex;align-items:center;gap:6px;margin:0;color:var(--text)">
            <input type="checkbox" id="udDry" checked> 维基干跑（只出 diff，不写入）
          </label>
          <div><button class="btn" id="udRun">开始数据页同步</button></div>
        </div>
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

  const submit = async (body, confirmText) => {
    if (confirmText && !confirm(confirmText)) return;
    const j = await postJSON("/api/jobs", body);
    location.hash = `#/jobs/${j.id}`;
  };

  const ulBtn = $("#ulRun");
  ulBtn.onclick = async () => {
    const dry = $("#ulDry").checked;
    ulBtn.disabled = true;
    try {
      await submit(
        { kind: "update_local", dry_run: dry },
        dry ? null : "update-local 将真实执行本地文件操作（scripts 换树、图片/动画归档）。确定继续？",
      );
    } catch (e) {
      ulBtn.disabled = false;
      alert("提交失败：" + e.message);
    }
  };

  const udBtn = $("#udRun");
  udBtn.onclick = async () => {
    const dry = $("#udDry").checked;
    const body = { kind: "update_data", wiki_dry_run: dry };
    const snap = $("#udSnap").value;
    if (snap) body.snapshot = snap;
    udBtn.disabled = true;
    try {
      await submit(
        body,
        dry ? null : "update-data 将真实写入维基（模块常量 / 配方 / 物品表 / Strings 桶页）。确定继续？",
      );
    } catch (e) {
      udBtn.disabled = false;
      alert("提交失败：" + e.message);
    }
  };
}
