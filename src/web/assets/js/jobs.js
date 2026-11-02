//! 任务列表与任务详情页
"use strict";

import { esc, fmtTime, getJSON, postJSON, statusBadge, elFromHtml, $, routeTimerRef } from "./util.js";

// 仅保留需要特殊控件/文案的作业布局；作业清单、标签、wiki 标记与参数
// schema 全部来自 GET /api/jobs/specs（JobSpec 单一真源）。没有布局的
// web_ui 作业按 ParamSpec 生成通用表单。
const JOB_UI = {
  maintain_copy_clip: {
    fields: [{ k: "type", label: "类型：rbtl / tech / filters / names（留空=全部）" }],
  },
  maintain_strings: {
    fields: [
      { k: "version", label: "版本前缀（页面名与索引名，默认 DST）", default: "DST" },
      { k: "snapshot", label: "scripts 快照目录名（可选，留空=最新）" },
      { k: "output", label: "本地产物目录（可选，默认 output/strings/<VERSION>）" },
      { k: "offline", type: "check", label: "跳过现网对比（无维基流量）" },
      { k: "bucket_count", type: "number", default: "100", label: "无现有索引时的等分桶数（默认 100）" },
      { k: "rebalance", type: "check", label: "忽略现有索引边界，强制等量重切" },
      { k: "limit", type: "number", default: "0", label: "Canary：最多写入 N 个桶页（0=不限制）" },
    ],
  },
  create_redirect: {
    fields: [
      { k: "from", label: "源页面（如 File:Wendy potion duration.png）" },
      { k: "to", label: "目标页面（如 File:Wendy potion 3.png）" },
      { k: "summary", label: "编辑摘要（可选，默认「创建重定向」）" },
    ],
  },
  maintain_template_check: {
    fields: [
      { k: "output", label: "可粘贴片段输出文件（可选，如 output/template_snippets.txt）" },
      { k: "skip_icon_status", type: "check", label: "跳过 live 图标存在性查询（只用本地 icon_meta）" },
    ],
  },
  skilltree_wiki: {
    fields: [
      { k: "character", label: "角色过滤子串（留空=全部，如 walter）" },
      { k: "output", label: "产物目录（可选，写 <Char>.lua 文件）" },
    ],
  },
  skilltree_export: {
    fields: [
      { k: "character", label: "角色过滤子串（留空=全部，如 walter）" },
      { k: "output", label: "输出目录（可选，默认 output/skilltree）" },
    ],
  },
  prefab_overrides: {
    fields: [{ k: "input", label: "Lua 文件路径" }],
  },
  prefab_overrides_dir: {
    fields: [{ k: "input", label: "Lua 目录路径（留空 = DST__ROOT/data/databundles/scripts/prefabs）" }],
  },
  prefab_overrides_audit: {
    fields: [
      { k: "scripts", label: "游戏脚本根目录（留空 = DST__ROOT/data/databundles/scripts）" },
      { k: "wiki_file", label: "线上页面原文文件（可选，留空 = 在线拉取）" },
      { k: "output", label: "审计报告 JSON 输出（可选）" },
    ],
  },
  maintain_prefab_overrides: {
    fields: [
      { k: "scripts", label: "游戏脚本根目录（留空 = DST__ROOT/data/databundles/scripts）" },
      { k: "wiki_file", label: "线上页面原文文件（可选，留空 = 在线拉取）" },
      { k: "output", label: "产物目录（可选：PrefabOverrides.lua + diff.json）" },
    ],
  },
  scripts_sync: {
    fields: [
      { k: "force", type: "check", label: "强制重新同步（版本相同也执行）" },
      { k: "dry_run", type: "check", label: "演练：只报告计划、不动任何文件" },
      { k: "state_path", label: "版本状态文件（可选，默认 ./dst_version.txt）", default: "./dst_version.txt" },
    ],
  },
  images_sync: {
    fields: [
      { k: "force", type: "check", label: "忽略增量与幂等检查，全量重跑（并重查上传状态）" },
      { k: "dry_run", type: "check", label: "演练：只盘点并报告计划、不写文件" },
      { k: "skip_wiki_status", type: "check", label: "跳过维基上传状态查询（离线/无凭据）" },
    ],
  },
  upload_icons: {
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
    fields: [
      { k: "force", type: "check", label: "强制重新归档（忽略幂等检查）" },
      { k: "dry_run", type: "check", label: "演练：只盘点并报告计划、不写文件" },
      { k: "label", label: "版本标签（可选，默认 version.txt）" },
      { k: "out", label: "历史根目录（可选，默认 $ANIM__OUT_DIR 或 output/anim）" },
    ],
  },
  anim_diff: {
    fields: [
      { k: "old", label: "旧动画目录" },
      { k: "new", label: "新动画目录" },
      { k: "zip", label: "只对比某个相对路径（可选，如 dynamic/abigail_ice.dyn）" },
    ],
  },
  skin_index: {
    fields: [
      { k: "scripts", label: "游戏脚本根目录（留空=DST__ROOT/data/databundles/scripts）" },
      { k: "anim", label: "动画资源目录（可选，默认由 scripts 推导）" },
      { k: "out", label: "输出 JSON（可选，默认 output/skin-index.json）" },
    ],
  },
  cooking_game_export: {
    fields: [
      { k: "output", label: "输出目录（可选，默认 output/cooking-game-bundle）" },
      { k: "snapshot", label: "脚本快照目录名（可选，默认最新）" },
      { k: "zip", type: "check", label: "同时生成同名 .zip" },
      { k: "allow_missing_icons", type: "check", label: "缺少图标时继续（默认报错）" },
    ],
  },
  update_local: {
    fields: [
      { k: "force", type: "check", label: "三步都忽略版本/幂等检查，强制重跑" },
      { k: "dry_run", type: "check", label: "演练：三步都只报告计划、不写任何文件" },
      { k: "skip_wiki_status", type: "check", label: "跳过 images-sync 的维基上传状态查询" },
      { k: "from", label: "从该步骤开始（scripts-sync / images-sync / anim-sync，留空=从头）" },
    ],
  },
  update_data: {
    fields: [
      { k: "snapshot", label: "四步共用的 scripts 快照目录名（可选，留空=最新/live 树）" },
      { k: "from", label: "从该步骤开始（copy-clip / dst-recipes / item-table / strings，留空=从头）" },
    ],
  },
  update_assets: {
    fields: [
      { k: "from", label: "从该步骤开始（upload-icons / skilltree-wiki，留空=从头）" },
    ],
  },
  update_check: {
    readonly: true,
    fields: [
      { k: "snapshot", label: "两步共用的 scripts 快照目录名（可选）" },
      { k: "skip_icon_status", type: "check", label: "跳过模板检查的图标存在性查询" },
      { k: "from", label: "从该步骤开始（maintain-template-check / prefab-overrides-audit）" },
    ],
  },
};

/* ---------------- shared helpers ---------------- */

function fmtDur(ms) {
  if (ms == null || ms < 0) return "—";
  const s = ms / 1000;
  if (s < 60) return `${s.toFixed(1)}s`;
  const m = Math.floor(s / 60);
  return `${m}m${String(Math.round(s % 60)).padStart(2, "0")}s`;
}

function truncate(s, n) {
  s = s || "";
  return s.length > n ? s.slice(0, n) + "…" : s;
}

function levelLabel(l) {
  return { debug: "调试", info: "信息", warn: "警告", error: "错误" }[l] || l;
}

/* ---------------- job list ---------------- */

export async function pageJobs(main) {
  // 作业清单、标签、wiki 标记与参数 schema 全部来自 JobSpec（单一真源）。
  const specs = (await getJSON("/api/jobs/specs")).jobs;
  // 选项值必须是 serde tag（snake）；`name` 是 CLI 的 kebab 名。
  const specByName = Object.fromEntries(specs.map(s => [s.tag, s]));

  main.innerHTML = `
    <div class="panel">
      <h2>提交新任务</h2>
      <div class="row">
        <select id="jobKind">${specs
          .map(s => `<option value="${esc(s.tag)}">${esc(s.label)}</option>`).join("")}</select>
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

  // 没有手写布局的 web_ui 作业：按 ParamSpec 生成通用表单。
  const genericFields = (spec) => (spec.params || []).map(p => ({
    k: p.key,
    label: (p.help || p.key) + (p.required ? "（必填）" : ""),
    type: p.kind === "bool" ? "check" : p.kind === "usize" ? "number" : p.kind === "str_list" ? "list" : "text",
    default: p.default,
    required: p.required,
  }));
  const fieldsFor = (name) => {
    const layout = JOB_UI[name] || {};
    return layout.fields || genericFields(specByName[name]);
  };
  const hasDryRunField = (fields) => (fields || []).some(f => (Array.isArray(f) ? f[0] : f.k) === "dry_run");

  const renderFields = () => {
    const name = kindSel.value;
    const spec = specByName[name];
    const layout = JOB_UI[name] || {};
    const fields = fieldsFor(name);
    $("#dryRunRow").style.display = spec.wiki ? "" : "none";
    $("#jobFields").innerHTML = fields.map(f => {
      const [k, label] = Array.isArray(f) ? f : [f.k, f.label];
      const rawDefault = !Array.isArray(f) && typeof f.default === "function"
        ? f.default()
        : (!Array.isArray(f) ? f.default : "");
      const defaultValue = rawDefault == null ? "" : rawDefault;
      const required = !Array.isArray(f) && f.required ? " required" : "";
      if (!Array.isArray(f) && f.type === "check")
        return `<label style="display:flex;align-items:center;gap:6px;margin:6px 0;color:var(--text)">
          <input type="checkbox" data-field="${k}" ${defaultValue === true || defaultValue === "true" ? "checked" : ""}> ${esc(label)}</label>`;
      if (!Array.isArray(f) && f.type === "number")
        return `<label>${esc(label)}<input type="number" min="0" step="1" style="width:100%" data-field="${k}" data-num="1" value="${esc(defaultValue)}"${required}></label>`;
      if (!Array.isArray(f) && f.type === "list")
        return `<label>${esc(label)}<input style="width:100%" data-field="${k}" data-list="1" placeholder="多个值用逗号分隔" value="${esc(defaultValue)}"${required}></label>`;
      return `<label>${esc(label)}<input style="width:100%" data-field="${k}" value="${esc(defaultValue)}"${required}></label>`;
    }).join("");
    const hasDryRun = hasDryRunField(fields);
    $("#jobHint").textContent = layout.readonly
      ? "只读核对：不写维基、不修改本地文件，可直接提交。"
      : spec.wiki
      ? "涉及维基写入的任务在维基干跑下只生成 diff 预览；取消勾选并经确认后直接写入。"
      : hasDryRun
        ? "纯本地任务不写维基；其参数中的“演练”勾选决定是否真实改动本地文件。"
        : "纯本地任务不写维基，直接在本地执行。";
  };
  kindSel.onchange = renderFields;
  renderFields();

  $("#submitJob").onclick = async () => {
    const name = kindSel.value;
    const spec = specByName[name];
    const layout = JOB_UI[name] || {};
    const fields = fieldsFor(name);

    // 页面不是 <form>，required 属性不会自动校验。
    const missing = Array.from(document.querySelectorAll("#jobFields input[required]"))
      .find(i => !i.value.trim());
    if (missing) {
      missing.focus();
      alert("请填写必填参数：" + (missing.closest("label")?.textContent.trim() || missing.dataset.field));
      return;
    }

    const body = { kind: name };
    if (spec.wiki) body.wiki_dry_run = $("#dryRun").checked;
    document.querySelectorAll("#jobFields input[data-field]").forEach(i => {
      if (i.type === "checkbox") { body[i.dataset.field] = i.checked; }
      else {
        const v = i.value.trim();
        if (v === "") return;
        if (i.dataset.num === "1") {
          const n = Number(v);
          if (Number.isFinite(n)) body[i.dataset.field] = n;
          return;
        }
        if (i.dataset.list === "1") {
          body[i.dataset.field] = v.split(/[,，]/).map(x => x.trim()).filter(Boolean);
          return;
        }
        body[i.dataset.field] = v;
      }
    });
    if (spec.wiki) {
      if (!body.wiki_dry_run && !confirm("已关闭维基干跑：任务可能直接修改维基页面。确定继续？")) return;
    } else if (hasDryRunField(fields) && !body.dry_run) {
      if (!confirm(`${spec.label} 将真实执行本地文件操作（非演练）。确定继续？`)) return;
    } else if (!layout.readonly && !hasDryRunField(fields)) {
      if (!confirm(`${spec.label} 将真实执行本地文件操作。确定继续？`)) return;
    }
    try {
      const j = await postJSON("/api/jobs", body);
      location.hash = `#/jobs/${j.id}`;
    } catch (e) { alert("提交失败：" + e.message); }
  };

  const refresh = async () => {
    const jobs = (await getJSON("/api/jobs")).jobs;
    $("#jobList").innerHTML = `<table><thead><tr>
        <th>状态</th><th>类型</th><th>进展</th><th>创建</th><th>耗时</th><th>告警</th>
      </tr></thead><tbody>${jobs.map(j => {
        const dur = j.finished_at_ms && j.started_at_ms
          ? fmtDur(j.finished_at_ms - j.started_at_ms)
          : (j.status === "running" ? "进行中…" : "—");
        const stage = j.stage_index && j.stage_total
          ? `步骤 ${j.stage_index}/${j.stage_total} · ${esc(j.last_stage || "")}`
          : (j.last_stage ? esc(j.last_stage) : "");
        const pct = j.progress && j.progress.total
          ? Math.round(100 * j.progress.done / j.progress.total) : null;
        const prog = pct === null ? "" : `<div class="mini-bar" title="${esc(j.progress.label || "")} ${j.progress.done}/${j.progress.total}"><i style="width:${pct}%"></i></div>`;
        const failLine = j.status === "failed" && j.error
          ? `<div class="ellipsis" style="color:var(--err)" title="${esc(j.error)}">${esc(truncate(j.error, 70))}</div>` : "";
        const last = failLine || (j.last_log
          ? `<div class="muted ellipsis" title="${esc(j.last_log)}">${esc(truncate(j.last_log, 70))}</div>` : "");
        const bad = j.error_count
          ? `<span class="badge b-err" title="错误 ${j.error_count} 条">${j.error_count}</span>`
          : j.warn_count
            ? `<span class="badge b-warn" title="警告 ${j.warn_count} 条">${j.warn_count}</span>` : "";
        return `<tr style="cursor:pointer" onclick="location.hash='#/jobs/${j.id}'">
          <td>${statusBadge(j.status)}</td>
          <td><code>${esc(j.kind)}</code>${j.touches_wiki && j.wiki_dry_run ? ' <span class="badge b-warn">干跑</span>' : ""}</td>
          <td class="progress-cell">${stage}${prog}${last}</td>
          <td>${fmtTime(j.created_at_ms)}</td><td>${dur}</td><td>${bad}</td></tr>`;
      }).join("") || '<tr><td colspan="6" class="muted">暂无任务</td></tr>'}</tbody></table>`;
  };
  await refresh();
  routeTimerRef.id = setInterval(refresh, 3000);
}

/* ---------------- job detail ---------------- */

export async function pageJobDetail(main, id) {
  main.innerHTML = `
    <div class="panel" id="jdHead"></div>
    <div class="panel">
      <div class="job-toolbar">
        <div class="tabs">
          <button class="tab active" data-tab="log">日志</button>
          <button class="tab" data-tab="diff" id="tabDiff">Diff 预览</button>
        </div>
        <span id="jdStep" class="muted"></span>
        <span id="jdStepBar" class="step-bar"><i></i></span>
        <span class="spacer"></span>
        <input id="logSearch" placeholder="搜索日志…" style="width:180px">
        <select id="logLevel" title="按级别过滤">
          <option value="all">全部级别</option>
          <option value="warn">警告 + 错误</option>
          <option value="error">仅错误</option>
        </select>
        <label style="margin:0;display:flex;align-items:center;gap:6px;color:var(--text);font-size:13px">
          <input type="checkbox" id="autoScroll" checked> 自动滚动</label>
        <button class="btn secondary mini" id="toggleStages">全部折叠</button>
        <button class="btn secondary mini" id="downloadLog">下载日志</button>
        <span id="logStats" class="muted"></span>
      </div>
      <div class="log" id="jdLog"></div>
      <div id="jdDiffs" style="display:none">
        <div class="row" style="margin-bottom:8px">
          <input id="diffSearch" placeholder="筛选页面…" style="width:220px">
          <span id="diffStats" class="muted"></span>
        </div>
        <div id="diffList"></div>
      </div>
    </div>`;

  const logEl = $("#jdLog");
  const diffEl = $("#jdDiffs");
  const diffListEl = $("#diffList");
  const diffStatsEl = $("#diffStats");
  const statsEl = $("#logStats");
  const stepEl = $("#jdStep");
  const tabDiffBtn = $("#tabDiff");
  const tabBtns = main.querySelectorAll(".tab");
  const state = {
    autoScroll: true,
    stage: null,
    step: null,
    stepBody: null,
    stages: 0,
    lines: 0,
    warn: 0,
    error: 0,
    diffs: 0,
    events: [],
    finished: 0,
    startedMs: 0,
    finishing: false,
  };

  tabBtns.forEach(b => b.onclick = () => {
    tabBtns.forEach(x => x.classList.toggle("active", x === b));
    const showDiff = b.dataset.tab === "diff";
    logEl.style.display = showDiff ? "none" : "";
    diffEl.style.display = showDiff ? "" : "none";
    if (!showDiff) maybeScroll();
  });

  /* ----- structured log rendering ----- */

  const updateStats = () => {
    statsEl.textContent = `${state.lines} 行` +
      (state.warn ? ` · ${state.warn} 警告` : "") +
      (state.error ? ` · ${state.error} 错误` : "");
  };

  function stageBlock(name, isStep) {
    const el = elFromHtml(`
      <details class="stage${isStep ? " step" : ""}" open>
        <summary>
          <span class="s-dot running"></span>
          ${isStep ? '<span class="step-no"></span>' : ""}
          <span class="s-name"></span>
          <span class="s-meta muted"></span>
        </summary>
        <div class="s-body"></div>
      </details>`);
    el.querySelector(".s-name").textContent = name;
    return el;
  }

  function closeStage(endTs) {
    const s = state.stage;
    if (!s) return;
    state.stage = null;
    // 空 stage（如编排入口的"写入模式"标题行）没有内容，直接移除，
    // 避免日志里出现一串 "0 行" 的空壳。
    if (s.count === 0 && !s.el.querySelector(".progress")) { s.el.remove(); return; }
    const dur = endTs ? endTs - s.start : Date.now() - s.start;
    const dot = s.el.querySelector(".s-dot");
    dot.classList.remove("running");
    dot.classList.add(s.error ? "bad" : s.warn ? "warn" : "ok");
    const meta = s.el.querySelector(".s-meta");
    meta.textContent = `${s.count} 行 · ${fmtDur(dur)}` + (s.warn ? ` · ${s.warn} 警告` : "") + (s.error ? ` · ${s.error} 错误` : "");
    // 只有长阶段自动折叠；任务结束时最后一个阶段保持展开，
    // 否则"未检测到变化 / 已写入 / 跳过"这类收尾提示会被收起来。
    if (!state.finishing && s.count > 20 && !s.warn && !s.error) s.el.open = false;
  }

  function closeStep(endTs) {
    const st = state.step;
    if (!st) return;
    const dur = endTs ? endTs - st.start : Date.now() - st.start;
    const meta = st.el.querySelector(".s-meta");
    const lines = st.el.querySelectorAll(".ll").length;
    meta.textContent = `${lines} 行 · ${fmtDur(dur)}`;
    const dot = st.el.querySelector(".s-dot");
    dot.classList.remove("running");
    dot.classList.add(st.error ? "bad" : st.warn ? "warn" : "ok");
    // 步骤头保持展开：折叠子阶段后，一行行"阶段 · 行数 · 耗时"就是时间线。
    state.step = null;
    state.stepBody = null;
    stepEl.textContent = "";
  }

  function openStage(ev) {
    // 新的普通 stage：挂在当前 step 里（若有），否则直接挂日志根。
    const isStep = ev.index != null;
    if (isStep) {
      closeStage(ev.ts_ms);
      closeStep(ev.ts_ms);
      const el = stageBlock(ev.name, true);
      el.querySelector(".step-no").textContent = `步骤 ${ev.index}/${ev.total}`;
      logEl.appendChild(el);
      state.step = { el, start: ev.ts_ms, warn: 0, error: 0 };
      state.stepBody = el.querySelector(".s-body");
      stepEl.textContent = `步骤 ${ev.index}/${ev.total}：${ev.name}`;
      const bar = $("#jdStepBar");
      if (bar) {
        bar.style.display = "";
        bar.querySelector("i").style.width = `${Math.round(100 * ev.index / ev.total)}%`;
      }
      return;
    }
    closeStage(ev.ts_ms);
    const parent = state.stepBody || logEl;
    const el = stageBlock(ev.name, false);
    parent.appendChild(el);
    state.stages++;
    state.stage = { el, start: ev.ts_ms, count: 0, warn: 0, error: 0, meta: el.querySelector(".s-meta") };
    el.querySelector(".s-meta").textContent = "0 行";
  }

  function ensureStage(ts) {
    if (!state.stage) openStage({ name: "输出", ts_ms: ts, index: null });
  }

  function addLine(ev) {
    ensureStage(ev.ts_ms);
    const line = document.createElement("div");
    line.className = `ll ${ev.level || "info"}`;
    line.dataset.level = ev.level || "info";
    line.textContent = ev.text;
    line.title = ev.text;
    state.stage.el.querySelector(".s-body").appendChild(line);
    state.stage.count++;
    state.lines++;
    if (ev.level === "warn") { state.stage.warn++; state.warn++; if (state.step) state.step.warn++; }
    if (ev.level === "error") { state.stage.error++; state.error++; if (state.step) state.step.error++; }
    applyFilter(line);
    updateStats();
    maybeScroll();
  }

  function addProgress(ev) {
    ensureStage(ev.ts_ms);
    const body = state.stage.el.querySelector(".s-body");
    let row = body.querySelector(".progress");
    if (!row) {
      row = elFromHtml(`<div class="progress"><span class="p-label"></span><span class="bar"><i></i></span><span class="p-num"></span></div>`);
      body.appendChild(row);
    }
    const pct = ev.total ? Math.min(100, Math.round(100 * ev.done / ev.total)) : 0;
    row.querySelector(".p-label").textContent = ev.label || "进度";
    row.querySelector(".bar i").style.width = `${pct}%`;
    row.querySelector(".p-num").textContent = ev.total ? `${ev.done}/${ev.total}` : String(ev.done);
  }

  function maybeScroll() {
    if (state.autoScroll) logEl.scrollTop = logEl.scrollHeight;
  }

  /* ----- filters ----- */

  const isVisible = (line) => {
    const lvl = $("#logLevel").value;
    if (lvl === "warn" && !["warn", "error"].includes(line.dataset.level)) return false;
    if (lvl === "error" && line.dataset.level !== "error") return false;
    const q = $("#logSearch").value.trim().toLowerCase();
    if (q && !line.textContent.toLowerCase().includes(q)) return false;
    return true;
  };

  function applyFilter(line) {
    line.hidden = !isVisible(line);
  }

  function applyFilters() {
    logEl.querySelectorAll(".ll").forEach(applyFilter);
    // 没有可见日志的 stage 整体隐藏（进度/步骤头保留）。
    logEl.querySelectorAll(".stage").forEach(st => {
      if (st.classList.contains("step")) return;
      const any = [...st.querySelectorAll(".ll")].some(l => !l.hidden);
      st.hidden = !any && st.querySelectorAll(".ll").length > 0;
    });
  }

  $("#logSearch").oninput = applyFilters;
  $("#logLevel").onchange = applyFilters;
  $("#diffSearch").oninput = applyDiffFilter;

  $("#autoScroll").onchange = () => { state.autoScroll = $("#autoScroll").checked; maybeScroll(); };
  logEl.addEventListener("scroll", () => {
    const atBottom = logEl.scrollHeight - logEl.scrollTop - logEl.clientHeight < 40;
    if (!atBottom && state.autoScroll) {
      state.autoScroll = false;
      $("#autoScroll").checked = false;
    }
  });

  $("#toggleStages").onclick = () => {
    const stages = [...logEl.querySelectorAll("details.stage")];
    const anyOpen = stages.some(s => s.open);
    stages.forEach(s => { s.open = !anyOpen; });
    $("#toggleStages").textContent = anyOpen ? "全部展开" : "全部折叠";
  };

  $("#downloadLog").onclick = () => {
    const text = state.events.map(ev => {
      const t = new Date(ev.ts_ms || 0).toLocaleTimeString();
      if (ev.type === "stage") return `\n[${t}] ===== ${ev.index ? `步骤 ${ev.index}/${ev.total}：` : ""}${ev.name} =====`;
      if (ev.type === "log") return `[${t}] ${levelLabel(ev.level)} ${ev.text}`;
      if (ev.type === "progress") return `[${t}] 进度 ${ev.label} ${ev.done}/${ev.total ?? "?"}`;
      if (ev.type === "diff") return `[${t}] diff ${ev.page} +${ev.added} -${ev.removed}`;
      if (ev.type === "done") return `[${t}] 结束：${ev.status}`;
      return "";
    }).join("\n");
    const blob = new Blob([text], { type: "text/plain;charset=utf-8" });
    const a = document.createElement("a");
    a.href = URL.createObjectURL(blob);
    a.download = `job-${id}.log`;
    a.click();
    setTimeout(() => URL.revokeObjectURL(a.href), 1000);
  };

  /* ----- diffs ----- */

  function addDiff(d, open) {
    state.diffs++;
    tabDiffBtn.textContent = `Diff 预览 (${state.diffs})`;
    const lines = (d.text || "").split("\n").map(l => {
      let cls = "";
      if (l.startsWith("+++") || l.startsWith("---")) cls = "d-head";
      else if (l.startsWith("@@")) cls = "d-hunk";
      else if (l.startsWith("+")) cls = "d-add";
      else if (l.startsWith("-")) cls = "d-del";
      return `<div class="${cls}">${esc(l)}</div>`;
    }).join("");
    const card = elFromHtml(`
      <details class="diff-card" ${open ? "open" : ""}>
        <summary class="diff-head">
          <code>${esc(d.page)}</code>
          <span class="badge b-ok">+${d.added}</span>
          <span class="badge b-err">-${d.removed}</span>
          <span class="spacer"></span>
          <span class="muted">${(d.text || "").split("\n").length} 行</span>
        </summary>
        <pre class="diff-body">${lines}</pre>
      </details>`);
    diffListEl.appendChild(card);
    const shown = diffListEl.querySelectorAll(".diff-card").length;
    diffStatsEl.textContent = `${shown} 个页面有变化`;
    applyDiffFilter();
  }

  function applyDiffFilter() {
    const q = ($("#diffSearch").value || "").trim().toLowerCase();
    let shown = 0;
    diffListEl.querySelectorAll(".diff-card").forEach(card => {
      const page = card.querySelector("code")?.textContent || "";
      const hit = !q || page.toLowerCase().includes(q);
      card.hidden = !hit;
      if (hit) shown++;
    });
    diffStatsEl.textContent = `${shown} / ${diffListEl.querySelectorAll(".diff-card").length} 个页面`;
  }

  /* ----- event pipeline ----- */

  function addEndBlock(ev) {
    const status = ev.status || "success";
    const cls = status === "success" ? "ok" : status === "failed" ? "bad" : "warn";
    const block = elFromHtml(`
      <div class="job-end ${cls}">
        <span class="s-dot ${cls}"></span>
        <b>任务结束：${esc(status)}</b>
        <span class="muted">耗时 ${elapsed()}</span>
        <span class="muted">${logEl.querySelectorAll("details.stage:not(.step)").length} 个阶段</span>
        ${state.warn ? `<span class="badge b-warn">${state.warn} 警告</span>` : ""}
        ${state.error ? `<span class="badge b-err">${state.error} 错误</span>` : ""}
      </div>`);
    logEl.appendChild(block);
    maybeScroll();
  }

  function handle(ev) {
    state.events.push(ev);
    if (ev.type === "log") addLine(ev);
    else if (ev.type === "stage") openStage(ev);
    else if (ev.type === "progress") addProgress(ev);
    else if (ev.type === "diff") addDiff(ev, state.diffs < 3);
    else if (ev.type === "done") {
      state.finished = ev.ts_ms;
      state.finishing = true;
      closeStage(ev.ts_ms);
      closeStep(ev.ts_ms);
      addEndBlock(ev);
      stopTimer();
    }
  }

  function resetView() {
    logEl.innerHTML = "";
    // 只清空 diff 卡片，保留筛选栏（#diffSearch/#diffStats/#diffList）。
    diffListEl.innerHTML = "";
    Object.assign(state, {
      stage: null, step: null, stepBody: null, stages: 0,
      lines: 0, warn: 0, error: 0, diffs: 0, events: [], finished: 0, finishing: false,
    });
    updateStats();
    tabDiffBtn.textContent = "Diff 预览";
    stepEl.textContent = "";
    diffStatsEl.textContent = "";
  }

  /* ----- header ----- */

  const elapsed = () => {
    if (!state.startedMs) return "—";
    return fmtDur((state.finished || Date.now()) - state.startedMs);
  };

  function tick() {
    const el = $("#jdElapsed");
    if (el) el.textContent = elapsed();
  }

  function stopTimer() {
    if (routeTimerRef.id) { clearInterval(routeTimerRef.id); routeTimerRef.id = null; }
  }

  function summaryChips(summary) {
    return Object.entries(summary || {})
      .filter(([, v]) => ["string", "number", "boolean"].includes(typeof v))
      .slice(0, 8)
      .map(([k, v]) => `<span class="chip"><span class="muted">${esc(k)}</span> ${esc(String(v))}</span>`)
      .join(" ");
  }

  function jsonTree(v, depth = 0) {
    if (v === null) return `<span class="j-null">null</span>`;
    const t = typeof v;
    if (t === "string") return `<span class="j-str">"${esc(v)}"</span>`;
    if (t === "number" || t === "boolean") return `<span class="j-num">${esc(String(v))}</span>`;
    if (Array.isArray(v)) {
      if (!v.length) return `<span class="muted">[]</span>`;
      const LIMIT = 20;
      const shown = v.slice(0, LIMIT)
        .map(x => `<div class="j-row">${jsonTree(x, depth + 1)}</div>`).join("");
      const more = v.length > LIMIT
        ? `<div class="muted">… 其余 ${v.length - LIMIT} 项未显示（用「下载 JSON」看完整内容）</div>` : "";
      return `<div class="j-arr">${shown}${more}</div>`;
    }
    if (t === "object") {
      if (depth > 8) return `<span class="muted">…</span>`;
      const keys = Object.keys(v);
      if (!keys.length) return `<span class="muted">{}</span>`;
      const rows = keys.slice(0, 200)
        .map(k => `<div class="j-row"><span class="j-key">${esc(k)}</span>: ${jsonTree(v[k], depth + 1)}</div>`).join("");
      const more = keys.length > 200 ? `<div class="muted">… 其余 ${keys.length - 200} 个键未显示</div>` : "";
      return `<div class="j-obj">${rows}${more}</div>`;
    }
    return esc(String(v));
  }

  function renderResult(h) {
    const r = h.result;
    if (!r) return "";
    const steps = r.details && Array.isArray(r.details.steps) ? r.details.steps : null;
    const raw = JSON.stringify(r, null, 2);
    const stepRows = steps ? `<div class="result-steps">${steps.map(s => {
      const st = s.result && s.result.status ? s.result.status : "—";
      return `<span class="chip"><span class="muted">${esc(s.step)}</span> ${statusBadge(st)}</span>`;
    }).join(" ")}</div>` : "";
    return `
      <div class="result-card">
        <div class="row">
          <b>执行结果</b>
          ${statusBadge(r.status || "success")}
          ${summaryChips(r.summary)}
          <span class="spacer"></span>
          <button class="btn secondary mini" id="copyJson">复制 JSON</button>
          <button class="btn secondary mini" id="downloadJson">下载 JSON</button>
        </div>
        ${stepRows}
        <details class="raw-json">
          <summary class="muted">原始 JSON（${raw.split("\n").length} 行）</summary>
          <div class="json-tree">${jsonTree(r)}</div>
        </details>
      </div>`;
  }

  function renderHead(h) {
    state.startedMs = h.started_at_ms || h.created_at_ms || 0;
    if (h.finished_at_ms) state.finished = h.finished_at_ms;
    const canApprove = h.touches_wiki && h.wiki_dry_run && h.status === "success";
    $("#jdHead").innerHTML = `
      <div class="row">
        <h2 style="margin:0"><code>${esc(h.kind)}</code> ${statusBadge(h.status)}</h2>
        <span class="muted">耗时 <b id="jdElapsed">${fmtDur((h.finished_at_ms || Date.now()) - (h.started_at_ms || h.created_at_ms || Date.now()))}</b></span>
        ${canApprove ? `<button class="btn" id="approveBtn">批准并真实写入</button>` : ""}
        ${h.status === "queued" ? `<button class="btn danger" id="cancelBtn">取消任务</button>` : ""}
        <button class="btn secondary" onclick="location.hash='#/jobs'">返回列表</button>
      </div>
      ${h.error ? `<p style="color:var(--err)">错误：${esc(h.error)}</p>` : ""}
      ${h.result && h.result.status === "rollback_skipped"
        ? `<p style="color:var(--warn)">检测到版本回退（${esc(h.result.details?.build || h.result.details?.label)} < 已记录 ${esc(h.result.details?.recorded_build || h.result.details?.recorded_label)}）：本次未执行任何操作，未写入任何文件。</p>`
        : ""}
      <details class="params"><summary class="muted">任务参数</summary><pre>${esc(JSON.stringify(h.params, null, 2))}</pre></details>
      ${renderResult(h)}`;

    const cb = $("#cancelBtn");
    if (cb) {
      cb.onclick = async () => {
        try { await postJSON(`/api/jobs/${id}/cancel`); }
        catch (e) { alert("取消失败：" + e.message); }
      };
      cb.disabled = h.status !== "queued";
      cb.title = h.status === "running" ? "运行中的任务不支持取消，请等待其完成" : "";
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
    const cj = $("#copyJson");
    if (cj) cj.onclick = () => navigator.clipboard.writeText(JSON.stringify(h.result, null, 2));
    const dj = $("#downloadJson");
    if (dj) dj.onclick = () => {
      const blob = new Blob([JSON.stringify(h.result, null, 2)], { type: "application/json" });
      const a = document.createElement("a");
      a.href = URL.createObjectURL(blob);
      a.download = `job-${id}-result.json`;
      a.click();
      setTimeout(() => URL.revokeObjectURL(a.href), 1000);
    };
  }

  /* ----- load + stream ----- */

  const head = await getJSON(`/api/jobs/${id}`);
  renderHead(head);
  for (const ev of head.logs || []) handle(ev);
  updateStats();
  const terminal = ["success", "failed", "cancelled"].includes(head.status);
  if (!terminal) {
    routeTimerRef.id = setInterval(tick, 1000);
    tick();
  }

  if (!terminal) {
    // `since` 是 seq 游标：服务端 replay/live 两侧按 seq 去重。
    const lastSeq = (head.logs || []).reduce((m, e) => Math.max(m, e.seq || 0), 0);
    const es = new EventSource(`/api/jobs/${id}/events?since=${lastSeq}`);
    es.onmessage = (msg) => {
      let ev = null;
      try { ev = JSON.parse(msg.data); } catch { return; }
      if (ev && ev.type === "resync") {
        // 广播滞后丢事件：重新拉全量并重建视图。
        (async () => {
          try {
            const h2 = await getJSON(`/api/jobs/${id}`);
            resetView();
            renderHead(h2);
            for (const e2 of h2.logs || []) handle(e2);
            updateStats();
            if (["success", "failed", "cancelled"].includes(h2.status)) {
              stopTimer();
              es.close();
            }
          } catch {}
        })();
        return;
      }
      try { handle(ev); } catch {}
      if (ev && ev.type === "done") {
        es.close();
        stopTimer();
        // Done 之后 result 才可能在 detail 中可见，稍后再刷新头部。
        setTimeout(async () => {
          try { renderHead(await getJSON(`/api/jobs/${id}`)); } catch {}
        }, 300);
      }
    };
    es.onerror = () => { /* EventSource 自动重连；终态时由 done 关闭 */ };
  }
}
