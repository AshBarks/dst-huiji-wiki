//! 任务列表与任务详情页
"use strict";

import { esc, fmtTime, api, getJSON, postJSON, elFromHtml, statusBadge, diffCardHtml, $ , routeTimerRef } from "./util.js";

const JOB_DEFS = {
  maintain_item_table: { label: "维护物品表 → 维基", wiki: true },
  maintain_dst_recipes: { label: "维护配方表 → 维基", wiki: true },
  maintain_copy_clip: {
    label: "维护模块常量 → 维基", wiki: true,
    fields: [{ k: "type", label: "类型：rbtl / tech / filters / names（留空=全部）" }],
  },
  create_redirect: {
    label: "创建页面重定向 → 维基", wiki: true,
    fields: [
      { k: "from", label: "源页面（如 File:Wendy potion duration.png）" },
      { k: "to", label: "目标页面（如 File:Wendy potion 3.png）" },
      { k: "summary", label: "编辑摘要（可选，默认「创建重定向」）" },
    ],
  },
  maintain_template_check: {
    label: "检查模板数据覆盖（只读）",
    fields: [
      { k: "output", label: "可粘贴片段输出文件（可选，如 output/template_snippets.txt）" },
      { k: "skip_icon_status", type: "check", label: "跳过 live 图标存在性查询（只用本地 icon_meta）" },
    ],
  },
  skilltree_wiki: {
    label: "维护技能树子页面 → 维基", wiki: true,
    fields: [
      { k: "character", label: "角色过滤子串（留空=全部，如 walter）" },
      { k: "output", label: "产物目录（可选，写 <Char>.lua 文件）" },
    ],
  },
  skilltree_export: {
    label: "技能树数据导出到本地",
    fields: [
      { k: "character", label: "角色过滤子串（留空=全部，如 walter）" },
      { k: "output", label: "输出目录（可选，默认 output/skilltree）" },
    ],
  },
  prefab_overrides: {
    label: "预制体重定向解析（单文件）",
    fields: [{ k: "input", label: "Lua 文件路径" }],
  },
  prefab_overrides_dir: {
    label: "预制体重定向解析（目录）",
    fields: [{ k: "input", label: "Lua 目录路径（留空 = DST__ROOT/data/databundles/scripts/prefabs）" }],
  },
  prefab_overrides_audit: {
    label: "预制体重定向审计（只读）",
    fields: [
      { k: "scripts", label: "游戏脚本根目录（留空 = DST__ROOT/data/databundles/scripts）" },
      { k: "wiki_file", label: "线上页面原文文件（可选，留空 = 在线拉取）" },
      { k: "output", label: "审计报告 JSON 输出（可选）" },
    ],
  },
  maintain_prefab_overrides: {
    label: "维护预制体重定向（只读 diff）",
    fields: [
      { k: "scripts", label: "游戏脚本根目录（留空 = DST__ROOT/data/databundles/scripts）" },
      { k: "wiki_file", label: "线上页面原文文件（可选，留空 = 在线拉取）" },
      { k: "output", label: "产物目录（可选：PrefabOverrides.lua + diff.json）" },
    ],
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
      { k: "force", type: "check", label: "忽略增量与幂等检查，全量重跑（并重查上传状态）" },
      { k: "dry_run", type: "check", label: "演练：只盘点并报告计划、不写文件" },
      { k: "skip_wiki_status", type: "check", label: "跳过维基上传状态查询（离线/无凭据）" },
    ],
  },
  upload_icons: {
    label: "upload-icons 上传图标 → 维基", wiki: true,
    fields: [
      { k: "build", label: "只上传首次加入该 build 的图标（留空=全部有英文名的）" },
      { k: "source", label: "只上传指定来源：inventory（物品栏）/ crafting（制作栏）/ skilltree（技能树图标），留空=全部" },
      { k: "file", label: "只上传单个文件名（如 axe.png，优先于 build）" },
      { k: "title", label: "手动指定 wiki 文件名（需配合 file，如 Pick-Axe.png；会写入映射表）" },
      { k: "only_missing", type: "check", default: true, label: "仅上传维基缺失的（批量）" },
      { k: "ignore_warnings", type: "check", label: "同名重传忽略警告（ignorewarnings=1）" },
      { k: "comment", label: "上传注释（可选）" },
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

export async function pageJobs(main) {
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
  routeTimerRef.id = setInterval(refresh, 3000);
}

export async function pageJobDetail(main, id) {
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
      ? `<p style="color:var(--warn)">检测到版本回退（${esc(h.result.details?.build || h.result.details?.label)} < 已记录 ${esc(h.result.details?.recorded_build || h.result.details?.recorded_label)}）：本次未执行任何操作，未写入任何文件。</p>`
      : ""}
    <details><summary class="muted">任务参数</summary><pre>${esc(JSON.stringify(h.params, null, 2))}</pre></details>
    ${h.result ? `<details open><summary>执行结果</summary><pre>${esc(JSON.stringify(h.result, null, 2))}</pre></details>` : ""}`;
    const cb = $("#cancelBtn");
    if (cb) {
      cb.onclick = async () => {
        try { await postJSON(`/api/jobs/${id}/cancel`); }
        catch (e) { alert("取消失败：" + e.message); }
      };
      // 运行中的任务不支持取消（避免硬中断批量写入的中间态）。
      cb.disabled = h.status !== "queued";
      cb.title = h.status === "running"
        ? "运行中的任务不支持取消，请等待其完成" : "";
    }
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
    // `since` 是 seq 游标（事件单调序号）：服务端对 replay/live 两侧按
    // seq 去重，刷新后重放不会重复。
    const lastSeq = (head.logs || []).reduce((m, e) => Math.max(m, e.seq || 0), 0);
    const es = new EventSource(`/api/jobs/${id}/events?since=${lastSeq}`);
    es.onmessage = (msg) => {
      let ev = null;
      try { ev = JSON.parse(msg.data); } catch {}
      if (ev && ev.type === "resync") {
        // 服务端广播滞后丢了事件：重新拉取全量日志并重放，Done 丢失也能恢复。
        (async () => {
          try {
            const h2 = await getJSON(`/api/jobs/${id}`);
            logEl.innerHTML = "";
            for (const e2 of h2.logs || []) appendEv(e2);
            for (const d2 of h2.diffs || []) appendDiff(d2);
            if (["success", "failed", "cancelled"].includes(h2.status)) {
              doneSeen = true;
              es.close();
              renderHead(h2);
            }
          } catch {}
        })();
        return;
      }
      try { appendEv(ev); } catch {}
      if (doneSeen) { es.close(); refreshHead(); }
    };
    es.onerror = () => { if (doneSeen) es.close(); };
  } else { doneSeen = true; }

  async function refreshHead() { renderHead(await getJSON(`/api/jobs/${id}`)); }
}

/* ---------------- recipes & ingredients ---------------- */
