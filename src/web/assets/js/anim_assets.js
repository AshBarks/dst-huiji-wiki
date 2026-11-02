//! 动画素材页
"use strict";

import { esc, api, getJSON, $ } from "./util.js";

export async function pageAnimAssets(main) {
  const state = {
    source: "prefab",
    tab: "prefab",
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
    follows: {},
    followTouched: new Set(),
    anchorSymbols: new Set(),
    followCandidates: {},
    followReports: [],
    followIndexVersion: 0,
    previewPlayer: null,
    previewRefreshTimer: null,
    previewSeq: 0,
    manualQ: "",
    manualResults: [],
    manualChecked: new Set(),
    manualPrimary: null,
    manualFiles: [],
  };

  async function apiJSON(url) { return getJSON(url); }

  main.innerHTML = `
    <div class="panel" style="margin-bottom:12px;padding:10px 12px">
      <div class="tabs" style="margin:0">
        <button class="tab active" data-asset-tab="prefab">Prefab 检索</button>
        <button class="tab" data-asset-tab="manual">手动文件</button>
      </div>
    </div>
    <div id="animPane"></div>`;
  const view = () => $("#animPane");

  // Stop playback and cancel pending refreshes before the detail DOM goes away;
  // otherwise the interval keeps touching detached nodes.
  const stopPreview = () => {
    state.previewSeq++;
    if (state.previewRefreshTimer) {
      clearTimeout(state.previewRefreshTimer);
      state.previewRefreshTimer = null;
    }
    if (state.previewPlayer) {
      state.previewPlayer.dispose();
      state.previewPlayer = null;
    }
  };

  /* ---------------- 名字显示 / 复制 / 左栏调宽 ---------------- */

  // `.zip` 与（Builds 的）`_build` 是整列公共后缀，列表里省略以把宽度让给
  // 有区分度的部分；完整路径始终保留在 title 与中栏文件栏里。
  function displayFileName(path, { build = false } = {}) {
    let name = String(path ?? "");
    if (name.endsWith(".zip")) name = name.slice(0, -4);
    if (build && name.endsWith("_build")) name = name.slice(0, -6);
    return name;
  }

  async function copyText(text) {
    try {
      await navigator.clipboard.writeText(text);
      return true;
    } catch {
      try {
        const ta = document.createElement("textarea");
        ta.value = text;
        ta.style.cssText = "position:fixed;top:0;left:0;opacity:0";
        document.body.appendChild(ta);
        ta.select();
        const ok = document.execCommand("copy");
        ta.remove();
        return ok;
      } catch {
        return false;
      }
    }
  }

  const LEFT_W_KEY = "anim-assets.left-width";
  const LEFT_W_DEFAULT = 360;
  const LEFT_W_MIN = 220;
  const LEFT_W_MAX = 460;

  function clampLeftWidth(w) {
    return Math.min(LEFT_W_MAX, Math.max(LEFT_W_MIN, Math.round(Number(w) || 0)));
  }

  function readLeftWidth() {
    try {
      const n = parseInt(localStorage.getItem(LEFT_W_KEY) ?? "", 10);
      return Number.isFinite(n) ? clampLeftWidth(n) : LEFT_W_DEFAULT;
    } catch {
      return LEFT_W_DEFAULT;
    }
  }

  function storeLeftWidth(w) {
    try { localStorage.setItem(LEFT_W_KEY, String(clampLeftWidth(w))); } catch { /* 隐私模式忽略 */ }
  }

  // 左栏宽度手柄：拖动实时改 --anim-left-w，松手落盘；双击/Home 复原。
  // 窄屏（<=1100px）由 CSS 接管栅格，手柄隐藏且拖动无效。
  function setupLeftResizer() {
    const grid = view().querySelector(".anim-detail-grid");
    const handle = view().querySelector(".anim-col-resize");
    if (!grid || !handle) return;
    const apply = (w) => {
      const px = clampLeftWidth(w);
      grid.style.setProperty("--anim-left-w", `${px}px`);
      handle.setAttribute("aria-valuenow", String(px));
    };
    const isNarrow = () => window.matchMedia("(max-width:1100px)").matches;
    const renderedWidth = () =>
      grid.querySelector(".anim-col-left")?.getBoundingClientRect().width || LEFT_W_DEFAULT;
    apply(readLeftWidth());

    handle.addEventListener("pointerdown", (e) => {
      if (e.button !== 0 || isNarrow()) return;
      e.preventDefault();
      const startX = e.clientX;
      const startW = renderedWidth();
      handle.setPointerCapture(e.pointerId);
      handle.classList.add("dragging");
      document.body.classList.add("anim-resizing");
      const onMove = (ev) => apply(startW + ev.clientX - startX);
      const onEnd = () => {
        handle.removeEventListener("pointermove", onMove);
        handle.removeEventListener("pointerup", onEnd);
        handle.removeEventListener("pointercancel", onEnd);
        handle.classList.remove("dragging");
        document.body.classList.remove("anim-resizing");
        try { handle.releasePointerCapture(e.pointerId); } catch { /* 已释放 */ }
        storeLeftWidth(renderedWidth());
      };
      handle.addEventListener("pointermove", onMove);
      handle.addEventListener("pointerup", onEnd);
      handle.addEventListener("pointercancel", onEnd);
    });

    handle.addEventListener("dblclick", () => {
      apply(LEFT_W_DEFAULT);
      storeLeftWidth(LEFT_W_DEFAULT);
    });

    handle.addEventListener("keydown", (e) => {
      const step = e.shiftKey ? 40 : 16;
      const cur = parseFloat(getComputedStyle(grid).getPropertyValue("--anim-left-w")) || LEFT_W_DEFAULT;
      if (e.key === "ArrowLeft") apply(cur - step);
      else if (e.key === "ArrowRight") apply(cur + step);
      else if (e.key === "Home") apply(LEFT_W_DEFAULT);
      else return;
      e.preventDefault();
      storeLeftWidth(parseFloat(getComputedStyle(grid).getPropertyValue("--anim-left-w")));
    });
  }

  main.querySelectorAll("[data-asset-tab]").forEach(btn => {
    btn.onclick = () => {
      state.tab = btn.dataset.assetTab;
      main.querySelectorAll("[data-asset-tab]").forEach(x =>
        x.classList.toggle("active", x === btn));
      if (state.tab === "prefab") renderList();
      else renderManual();
    };
  });

  async function search() {
    const kw = state.q.trim();
    const url = `/api/anim/assets/prefabs${kw ? `?q=${encodeURIComponent(kw)}` : ""}`;
    const r = await apiJSON(url);
    state.items = r.items || [];
    renderList();
  }

  function renderList() {
    stopPreview();
    view().innerHTML = `
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
      <div class="result-row">
        <div class="result-main">
          <div class="result-file"><code>${esc(file)}</code></div>
          <div class="result-meta">
            ${items.map(i => `<span class="chip">${esc(i.prefab_name || "?")}${i.skin_count ? ` <span class="muted">· skins ${i.skin_count}</span>` : ""}</span>`).join("")}
            <span class="muted">(${(items[0].anims || []).length} 动画文件 / ${items[0].build_file_count || 0} build 文件)</span>
          </div>
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
    state.source = "prefab";
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
    state.follows = {};
    state.followTouched = new Set();
    state.anchorSymbols = new Set();
    state.followCandidates = {};
    state.followReports = [];
    state.followIndexVersion = 0;
    const r = await apiJSON(`/api/anim/assets/prefab/${encodeURIComponent(file)}`);
    state.prefabItems = r.items || [];
    state.fileInfo = r.files || [];
    state.buildFiles = r.build_files || {};
    state.skins = r.skins || [];
    state.followIndexVersion = Number(r.follow_index_version) || 0;
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
    const buildFileCount = relatedFiles
      .concat(animFiles)
      .filter(path => (fileContent(path).builds || []).length > 0)
      .length;
    view().innerHTML = `
      <div class="panel">
        <div class="row">
          <button class="btn secondary" id="backToList">返回搜索</button>
          <h2 style="margin:0"><code>${esc(state.prefabFile)}</code></h2>
        </div>
      </div>
      <div class="card-row">
        <div class="card"><div class="num">${animFiles.length}</div><span class="muted">动画文件</span></div>
        <div class="card"><div class="num">${buildFileCount}</div><span class="muted">Build 文件</span></div>
      </div>
      <div class="panel">
        <h2>Animations 按文件</h2>
        ${animFiles.map(path => {
          const c = fileContent(path);
          const banks = c.banks || [];
          const animations = c.animations || [];
          const banksHtml = banks.map(b => `
            <div style="margin:2px 0 4px 10px">
              <b>${esc(b)}</b>
              <div style="margin-left:10px;display:flex;flex-wrap:wrap;gap:4px">${animations.map(a => `
                <button class="btn link" data-play="${esc(path)}|${esc(b)}|${esc(a)}">${esc(a)}</button>
              `).join("") || '<span class="muted">无动画</span>'}</div>
            </div>`).join("");
          return `<div class="file-block"><code>${esc(path)}</code><div style="margin-top:4px">${banksHtml || '<span class="muted">无 banks</span>'}</div></div>`;
        }).join("") || '<p class="muted">无动画文件</p>'}
      </div>
      <div class="panel">
        <h2>Builds 与 Symbols / Atlases</h2>
        ${relatedFiles.concat(animFiles).map(path => {
          const c = fileContent(path);
          if (!c.builds && !c.symbols && !c.atlases) return "";
          const disabled = state.disabledBuilds.has(path);
          return `<div class="file-block">
            <label style="display:flex;align-items:center;gap:6px;margin:0"><input type="checkbox" data-build-toggle="${esc(path)}" ${disabled ? "" : "checked"}> <code>${esc(path)}</code></label>
            <div class="muted" style="margin-top:4px">
              ${(c.builds || []).map(x => `build: ${esc(x)}`).join(", ")}
              <div>symbols: ${(c.symbols || []).slice(0, 50).map(esc).join(", ") || "—"}</div>
              <div>atlases: ${(c.atlases || []).map(esc).join(", ") || "—"}</div>
            </div>
          </div>`;
        }).join("") || '<p class="muted">无 build 文件</p>'}
      </div>`;

    $("#backToList").onclick = () => { state.prefabFile = null; renderList(); };
    view().querySelectorAll("[data-play]").forEach(btn => {
      btn.onclick = () => {
        const [file, bank, anim] = btn.dataset.play.split("|");
        openAnimation(file, bank, anim);
      };
    });
    view().querySelectorAll("[data-build-toggle]").forEach(chk => {
      chk.onchange = () => {
        const path = chk.dataset.buildToggle;
        if (chk.checked) state.disabledBuilds.delete(path);
        else state.disabledBuilds.add(path);
      };
    });
  }

  function sessionFiles(primary) {
    const base = state.source === "manual"
      ? state.manualFiles.slice()
      : state.relatedFiles.concat(state.animFiles.filter(f => f !== primary));
    return base.filter(f => f !== primary).concat([primary]);
  }

  // FollowSymbol anchors: an animated symbol with no build sprite whose
  // position drives a separate FX entity in the game.
  function anchorLooksLikeFollow(sym) {
    return /_swap$/.test(sym) || /(^|_)follow(_|$)/.test(sym);
  }

  function followFileForBank(bank) {
    if (!bank || !state.selectedFile) return state.selectedFile;
    const hasBank = (path) => (fileContent(path).banks || [])
      .some(name => String(name).toLowerCase() === String(bank).toLowerCase());
    if (hasBank(state.selectedFile)) return state.selectedFile;
    for (const path of sessionFiles(state.selectedFile)) {
      if (path !== state.selectedFile && hasBank(path)) return path;
    }
    return state.selectedFile;
  }

  function followSpecFromRecord(record, symbol) {
    const offset = Array.isArray(record.offset) ? record.offset : [0, 0];
    return {
      anchor: symbol,
      bank: record.bank || "",
      animation: record.animation || "",
      build: record.build || null,
      file: followFileForBank(record.bank),
      offset: [Number(offset[0]) || 0, Number(offset[1]) || 0],
      phase: 0,
      frame_begin: record.frame_begin ?? null,
      frame_end: record.frame_end ?? null,
      final_offset: record.final_offset ?? null,
      mult_colour: record.mult_colour ?? null,
      add_colour: record.add_colour ?? null,
      sync_frame: !!record.sync_frame,
      random_frame: !!record.random_frame,
      enabled: true,
    };
  }

  function followSpecFromCandidate(candidate, symbol) {
    return {
      anchor: symbol,
      bank: candidate.bank || "",
      animation: candidate.animation || "",
      build: null,
      file: candidate.file || state.selectedFile,
      offset: [0, 0],
      phase: 0,
      frame_begin: null,
      frame_end: null,
      final_offset: null,
      mult_colour: null,
      add_colour: null,
      sync_frame: false,
      random_frame: false,
      enabled: true,
    };
  }

  function refreshFollowSpecs() {
    const info = state.info;
    if (!info) return;
    const records = [];
    for (const item of state.prefabItems || []) {
      for (const record of (item.follow_fx || [])) records.push(record);
    }
    const byAnchor = new Map();
    for (const record of records) {
      const key = String(record.anchor || "").toLowerCase();
      if (!key) continue;
      if (!byAnchor.has(key)) byAnchor.set(key, []);
      byAnchor.get(key).push(record);
    }
    const previous = state.follows || {};
    const candidates = info.follow_candidates || {};
    const buildSymbols = new Set();
    for (const build of (info.builds || [])) {
      for (const symbol of (build.symbols || [])) buildSymbols.add(String(symbol).toLowerCase());
    }
    const next = {};
    const anchors = new Set();
    for (const raw of (info.symbols || [])) {
      const key = String(raw).toLowerCase();
      if (state.followTouched.has(key) && Object.prototype.hasOwnProperty.call(previous, key)) {
        next[key] = previous[key];
        if (previous[key]) anchors.add(key);
        continue;
      }
      const allRecs = byAnchor.get(key) || [];
      const recs = allRecs.filter(record => !record.non_sprite);
      // 记录存在但全是粒子/光源 FX：精灵合成不可能，不建挂点（也不走名字启发式）。
      if (allRecs.length && !recs.length) continue;
      const resolved = recs.find(record => record.animation);
      if (resolved) {
        anchors.add(key);
        next[key] = { ...followSpecFromRecord(resolved, raw), source: "prefab" };
        continue;
      }
      const cands = candidates[key] || [];
      if (cands.length) {
        anchors.add(key);
        next[key] = {
          ...followSpecFromCandidate(cands[0], raw),
          source: recs.length ? "prefab+heuristic" : "heuristic",
          candidates: cands,
        };
      } else if (!buildSymbols.has(key) && (recs.length || anchorLooksLikeFollow(key))) {
        anchors.add(key);
        next[key] = null;
      }
    }
    state.follows = next;
    state.anchorSymbols = anchors;
    state.followCandidates = candidates;
    state.followReports = [];
  }

  function followAnimChoices() {
    const out = [];
    const seen = new Set();
    if (!state.selectedFile) return out;
    for (const path of sessionFiles(state.selectedFile)) {
      const content = fileContent(path);
      const banks = content.banks || [];
      const animations = content.animations || [];
      for (const bank of banks) {
        for (const anim of animations) {
          const key = `${path}|${bank}|${anim}`;
          if (seen.has(key)) continue;
          seen.add(key);
          out.push({ file: path, bank, anim });
        }
      }
    }
    return out;
  }

  async function openAnimation(file, bank, anim) {
    state.selectedFile = file;
    state.selectedBank = bank;
    state.selectedAnim = anim;
    state.info = null;
    const files = sessionFiles(file);
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
    refreshFollowSpecs();
    renderAnimationDetail();
  }

  function renderAnimationDetail() {
    stopPreview();
    const info = state.info;
    if (!info) return;
    const symbolSet = info.symbols || [];
    const builds = info.builds || [];
    const frames = info.frames || [];

    view().innerHTML = `
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
            <button class="btn secondary" id="backToPrefab">${state.source === "manual" ? "返回手动文件" : "返回搜索"}</button>
            <b class="anim-anim-label" title="${esc(`${info.bank} / ${info.animation}`)}"><code>${esc(info.bank)} / ${esc(info.animation)}</code></b>
          </div>
          <div class="anim-file-bar" title="${esc(state.selectedFile || "")}">
            <span class="muted">文件</span>
            <code class="anim-file-path" id="animFilePath">${esc(state.selectedFile || "—")}</code>
            <button class="btn link mini" id="copyAnimFile" title="复制完整文件路径">复制</button>
          </div>
          <div id="animPreview" style="flex:1;width:100%;height:100%;min-height:0;overflow:auto;display:flex;align-items:center;justify-content:center;background:repeating-conic-gradient(#fff 0 25%, #eee 0 50%) 0 0/16px 16px;border:1px solid var(--border);margin:8px 0;position:relative"></div>
          <div class="row" style="justify-content:center;align-items:center;gap:6px;margin-bottom:6px">
            <button class="btn secondary" id="prevFrame" disabled title="上一帧">上一帧</button>
            <button class="btn" id="playPause" disabled title="播放 / 暂停">播放</button>
            <button class="btn secondary" id="nextFrame" disabled title="下一帧">下一帧</button>
            <span id="frameCounter" class="muted" style="min-width:180px;text-align:left"></span>
          </div>
          <div class="row" style="justify-content:center;gap:8px">
            <button class="btn" id="exportGif">导出 GIF</button>
            <button class="btn secondary" id="exportPng">导出 PNG 序列</button>
          </div>
          <div id="followStatus" class="muted" style="font-size:12px;text-align:center;margin-top:4px"></div>
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
        <div class="anim-col-resize" role="separator" aria-orientation="vertical"
          aria-label="调整左栏宽度" aria-valuemin="220" aria-valuemax="460" tabindex="0"
          title="拖动调整左栏宽度（双击复原，方向键微调）"></div>
      </div>`;

    $("#backToPrefab").onclick = () => (state.source === "manual" ? renderManual() : renderList());

    setupLeftResizer();

    const copyFileBtn = $("#copyAnimFile");
    if (copyFileBtn) copyFileBtn.onclick = async () => {
      const ok = await copyText(state.selectedFile || "");
      copyFileBtn.textContent = ok ? "已复制" : "复制失败";
      setTimeout(() => { copyFileBtn.textContent = "复制"; }, 1200);
    };

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
        <summary class="anim-group-summary" title="${esc(file)}"><code>${esc(displayFileName(file))}</code></summary>
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
      const atlases = b.atlases || [];
      return `<details data-build-details="${esc(b.file)}" style="padding:1px 0">
        <summary class="list-row" style="cursor:pointer" title="${esc(`${b.file} · ${syms.length} symbols / ${atlases.length} atlases`)}">
          <input type="checkbox" data-build-toggle="${esc(b.file)}" ${disabled ? "" : "checked"}>
          <code class="list-name" title="${esc(b.file)}">${esc(displayFileName(b.file, { build: true }))}</code>
        </summary>
        <div style="margin:4px 0 6px 18px">
          <input type="text" placeholder="过滤 symbol…" data-build-filter="${esc(b.file)}" style="width:100%">
          <div class="muted" style="font-size:12px;margin-top:2px">${syms.length} symbols · atlases: ${atlases.map(esc).join(", ") || "—"}</div>
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

    // One-click find build packages for missing symbols. Attachment anchors
    // are not build symbols: they are completed by composing a follower FX
    // animation instead, so they must not be sent to the build scan.
    const missingSymbols = symbolSet.filter(sym =>
      !state.anchorSymbols.has(sym.toLowerCase()) &&
      !builds.some(b => (b.symbols || []).some(x => x.toLowerCase() === sym.toLowerCase()))
    );

    const renderCandidates = () => {
      const list = state.candidateBuilds || [];
      $("#candidateBuilds").innerHTML = list.length === 0
        ? '<p class="muted">未搜索候选 build</p>'
        : list.map(b => {
            const checked = state.extraBuilds.has(b.file);
            const disabled = state.disabledBuilds.has(b.file);
            const via = b.via === "symbol_map"
              ? '<span class="badge b-ok">map</span>'
              : b.via === "both"
                ? '<span class="badge b-ok">map+同名</span>'
                : "";
            const matched = (b.matched_symbols || []).join(",")
              || (b.remaps || []).map(r => `${r.symbol}→${r.src_symbol}`).join(",");
            return `<div class="list-row" title="${esc(`${b.file}${matched ? ` · ${matched}` : ""}`)}">
              <input type="checkbox" data-candidate-toggle="${esc(b.file)}" ${checked ? "checked" : ""} ${disabled ? "disabled" : ""}>
              <code class="list-name" title="${esc(b.file)}">${esc(displayFileName(b.file, { build: true }))}</code>
              ${via}
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
        const known = new Set(state.candidateBuilds.map(b => b.file));
        for (const b of r.builds || []) {
          if (!known.has(b.file)) { state.candidateBuilds.push(b); known.add(b.file); }
          state.extraBuilds.add(b.file);
        }
        renderCandidates();
        renderSymbolDeps();
        schedulePreviewRefresh();
      } catch (e) {
        $("#candidateBuilds").innerHTML = `<p style="color:var(--err)">失败：${esc(e.message)}</p>`;
      }
    };
    $("#findMissingBuilds").onclick = loadCandidates;

    // 单个 symbol 的按需扫描：同名 provider ∪ symbol map 目标 build。
    const scanSymbolBuilds = async (sym) => {
      try {
        const r = await apiJSON(`/api/anim/assets/find-builds?symbols=${encodeURIComponent(sym)}`);
        const known = new Set(state.candidateBuilds.map(b => b.file));
        for (const b of r.builds || []) {
          if (!known.has(b.file)) { state.candidateBuilds.push(b); known.add(b.file); }
          state.extraBuilds.add(b.file);
        }
        renderCandidates();
        renderSymbolDeps();
        schedulePreviewRefresh();
      } catch (e) {
        alert("扫描失败：" + e.message);
      }
    };
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
      view().querySelectorAll("#leftAnims details").forEach(d => d.open = false);
    };
    $("#collapseSymbols").onclick = () => {
      view().querySelectorAll("#symbolDeps details").forEach(d => d.open = false);
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
      const idx = parseInt(e.target.value, 10) || 0;
      if (state.previewPlayer) state.previewPlayer.seek(idx);
      else renderFrameDetail(idx);
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

    const renderAnchorRow = (sym, follow, choices) => {
      const key = sym.toLowerCase();
      const enabled = follow ? follow.enabled !== false : false;
      const selectedKey = follow ? `${follow.file}|${follow.bank}|${follow.animation}` : "";
      const source = !follow
        ? "未解析"
        : follow.source === "prefab"
          ? "Lua FollowSymbol"
          : follow.source === "manual"
            ? "手动指定"
            : "同 bank 启发式";
      const options = choices.map(c => {
        const value = `${c.file}|${c.bank}|${c.anim}`;
        return `<option value="${esc(value)}" ${value === selectedKey ? "selected" : ""}>${esc(displayFileName(c.file))} · ${esc(c.bank)} / ${esc(c.anim)}</option>`;
      }).join("");
      const candidates = (state.followCandidates || {})[key] || [];
      const candidateHint = candidates.length > 1 && !(follow && follow.source === "prefab")
        ? `<div class="muted" style="font-size:11px">候选：${candidates.slice(0, 4).map(c => esc(c.animation)).join(", ")}</div>`
        : "";
      const badges = [];
      if (follow && follow.sync_frame) badges.push('<span class="badge b-ok">同步帧</span>');
      if (follow && follow.random_frame) badges.push('<span class="badge">随机起始帧</span>');
      if (follow && follow.final_offset) {
        badges.push(`<span class="badge">offset ${follow.final_offset > 0 ? "前" : "后"}${Math.abs(follow.final_offset)}</span>`);
      }
      if (follow && (follow.mult_colour || follow.add_colour)) badges.push('<span class="badge">染色</span>');
      const phase = follow && Number.isFinite(follow.phase) ? follow.phase : 0;
      const phaseHtml = follow ? `
              <div class="row" style="gap:6px;align-items:center;margin-top:4px">
                <span class="muted" style="font-size:11px">相位</span>
                <input type="number" data-follow-phase="${esc(sym)}" value="${phase}" style="width:72px;font-size:12px" ${follow.sync_frame ? "disabled" : ""}>
                <button class="btn link" data-follow-random="${esc(sym)}" ${follow.sync_frame ? "disabled" : ""}>随机</button>
                ${follow.sync_frame ? '<span class="muted" style="font-size:11px">跟随帧已与主动画同步</span>' : ""}
              </div>` : "";
      return `
        <div class="list-row">
          <input type="checkbox" data-follow-toggle="${esc(sym)}" ${enabled ? "checked" : ""} ${follow ? "" : "disabled"}>
          <details style="flex:1;min-width:0">
            <summary style="cursor:pointer;display:flex;align-items:center;gap:4px">
              <code class="list-name" style="flex:0 1 auto" title="${esc(sym)}">${esc(sym)}</code>
              <span class="badge b-ok">anchor</span>
              ${follow ? `<span class="muted" style="font-size:12px">↳ ${esc(follow.animation)}</span>` : '<span class="badge b-warn">未解析</span>'}
              ${badges.join("")}
            </summary>
            <div style="margin-left:10px;padding:2px 0">
              <select data-follow-anim="${esc(sym)}" style="width:100%;font-size:12px;padding:4px 6px">
                <option value="">— 不挂接 —</option>
                ${options}
              </select>
              ${candidateHint}
              <div class="muted" style="font-size:11px">跟随 FX · 来源：${esc(source)}</div>
              ${phaseHtml}
            </div>
          </details>
        </div>`;
    };

    const renderSymbolDeps = () => {
      const allBuilds = builds.concat((state.candidateBuilds || []).filter(b => state.extraBuilds.has(b.file)));
      const followChoices = followAnimChoices();
      const symHtml = symbolSet.map(sym => {
        const key = sym.toLowerCase();
        if (state.anchorSymbols.has(key)) {
          return renderAnchorRow(sym, state.follows[key] || null, followChoices);
        }
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
                <code class="list-name" style="flex:0 1 auto" title="${esc(sym)}">${esc(sym)}</code>${hidden ? '<span class="badge b-warn">hidden</span>' : ""}${autoIdx >= 0 ? '<span class="badge b-ok">auto-map</span>' : ""}
                <button class="btn link" data-sym-scan="${esc(sym)}" title="扫描含该 symbol（同名 ∪ symbol map）的可用 build">扫描 build</button>
              </summary>
              <div style="margin-left:10px;padding:2px 0">
                ${providers.length === 0 ? '<span class="muted">missing — no build provides this symbol</span>' : providers.map(b => `
                  <label title="${esc(`${b.file}${b.name ? ` · ${b.name}` : ""}${b.skin ? " · skin" : ""}`)}"
                    style="display:flex;align-items:center;gap:6px;padding:1px 0;font-size:12px">
                    <input type="radio" name="sym-${esc(sym)}" value="${esc(b.file)}"
                      data-sym-build="${esc(sym)}" ${effectiveChosen === b.file ? "checked" : ""}
                      ${state.disabledBuilds.has(b.file) ? "disabled" : ""}>
                    <code class="list-name" title="${esc(b.file)}">${esc(displayFileName(b.file, { build: true }))}</code>
                  </label>`).join("")}
                ${remapHtml}
              </div>
            </details>
          </div>`;
      }).join("");
      const staleIndex = state.source === "prefab" && state.followIndexVersion < 3
        ? '<p style="color:var(--warn);font-size:12px;margin:0 0 6px">当前 anim-index 未包含最新 FollowSymbol 数据（旧产物），挂点仅用启发式候选；建议重跑 <code>anim-index</code>。</p>'
        : "";
      $("#symbolDeps").innerHTML = staleIndex + (symHtml || '<p class="muted">无 symbol 依赖</p>');

      view().querySelectorAll("[data-sym-toggle]").forEach(chk => {
        chk.onchange = () => {
          const sym = chk.dataset.symToggle;
          if (chk.checked) state.hiddenSymbols.delete(sym);
          else state.hiddenSymbols.add(sym);
          renderSymbolDeps();
          schedulePreviewRefresh();
        };
      });
      view().querySelectorAll("[data-sym-build]").forEach(radio => {
        radio.onchange = () => {
          const sym = radio.dataset.symBuild;
          state.symbolBuilds[sym] = radio.value;
          delete state.remapChoice[sym];
          state.remapOptOut.delete(sym);
          renderSymbolDeps();
          schedulePreviewRefresh();
        };
      });
      view().querySelectorAll("[data-sym-remap]").forEach(sel => {
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
      view().querySelectorAll("[data-sym-scan]").forEach(btn => {
        btn.addEventListener("click", (e) => {
          e.preventDefault();
          e.stopPropagation();
          scanSymbolBuilds(btn.dataset.symScan);
        });
      });
      view().querySelectorAll("[data-follow-toggle]").forEach(chk => {
        chk.onchange = () => {
          const key = chk.dataset.followToggle.toLowerCase();
          const spec = state.follows[key];
          if (!spec) return;
          spec.enabled = chk.checked;
          state.followTouched.add(key);
          schedulePreviewRefresh();
        };
      });
      view().querySelectorAll("[data-follow-anim]").forEach(sel => {
        sel.onchange = () => {
          const key = sel.dataset.followAnim.toLowerCase();
          state.followTouched.add(key);
          state.anchorSymbols.add(key);
          if (!sel.value) {
            state.follows[key] = null;
          } else {
            const [file, bank, animation] = sel.value.split("|");
            state.follows[key] = {
              ...followSpecFromCandidate({ file, bank, animation }, key),
              source: "manual",
            };
          }
          renderSymbolDeps();
          schedulePreviewRefresh();
        };
      });
      view().querySelectorAll("[data-follow-phase]").forEach(input => {
        input.onchange = () => {
          const key = input.dataset.followPhase.toLowerCase();
          const spec = state.follows[key];
          if (!spec) return;
          spec.phase = Number.parseInt(input.value, 10) || 0;
          state.followTouched.add(key);
          schedulePreviewRefresh();
        };
      });
      view().querySelectorAll("[data-follow-random]").forEach(btn => {
        btn.addEventListener("click", (e) => {
          e.preventDefault();
          e.stopPropagation();
          const key = btn.dataset.followRandom.toLowerCase();
          const spec = state.follows[key];
          if (!spec) return;
          spec.phase = Math.floor(Math.random() * 1000);
          state.followTouched.add(key);
          renderSymbolDeps();
          schedulePreviewRefresh();
        });
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
      const files = extra.concat(sessionFiles(state.selectedFile));
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
      const follows = [];
      for (const [key, spec] of Object.entries(state.follows || {})) {
        if (!spec || spec.enabled === false || !spec.animation) continue;
        follows.push({
          anchor: key,
          bank: spec.bank || "",
          animation: spec.animation,
          file: spec.file || state.selectedFile,
          build: spec.build || null,
          offset: Array.isArray(spec.offset) ? spec.offset : null,
          phase: spec.phase || 0,
          frame_begin: spec.frame_begin ?? null,
          frame_end: spec.frame_end ?? null,
          final_offset: spec.final_offset ?? null,
          mult_colour: spec.mult_colour ?? null,
          add_colour: spec.add_colour ?? null,
          sync_frame: !!spec.sync_frame,
          random_frame: !!spec.random_frame,
        });
      }
      if (follows.length) params.set("follows", JSON.stringify(follows));
      if (state.selectedSkin) {
        params.set("skin_zip", state.selectedSkin.zip || "");
        if (state.selectedSkin.dyn) params.set("skin_dyn", state.selectedSkin.dyn);
      }
      return params.toString();
    };

    // Center preview player: the info timeline is authoritative, and rendered
    // images are keyed by original animation frame index so skipped frames stay
    // as blank slots instead of shifting the counter and frame details.
    const previewArea = $("#animPreview");
    const prevBtn = $("#prevFrame");
    const playBtn = $("#playPause");
    const nextBtn = $("#nextFrame");
    let zoom = 1;
    previewArea.addEventListener("wheel", (e) => {
      e.preventDefault();
      zoom = Math.min(4, Math.max(0.2, zoom * (e.deltaY > 0 ? 0.9 : 1.1)));
      const img = $("#animFrame");
      if (img) img.style.transform = `scale(${zoom})`;
    }, { passive: false });

    const setControlsEnabled = (enabled) => {
      prevBtn.disabled = !enabled;
      playBtn.disabled = !enabled;
      nextBtn.disabled = !enabled;
    };
    setControlsEnabled(false);

    const createPlayer = ({ images, fps, startPos, startPlaying }) => {
      const total = frames.length;
      const maxPos = total - 1;
      const clampPos = (p) => Math.max(0, Math.min(maxPos, p));
      const fpsLabel = Number(fps.toFixed(2));
      const player = {
        pos: 0,
        playing: false,
        timer: null,
        updateControls() {
          prevBtn.disabled = this.pos <= 0;
          nextBtn.disabled = this.pos >= maxPos;
          playBtn.disabled = total <= 1;
          playBtn.textContent = this.playing ? "暂停" : "播放";
          $("#frameCounter").textContent =
            `${this.pos + 1} / ${total} · ${fpsLabel} fps · ${this.playing ? "播放中" : "已暂停"}`;
        },
        renderAt(p) {
          this.pos = clampPos(p);
          const frame = frames[this.pos];
          const img = $("#animFrame");
          const empty = $("#animEmpty");
          const src = frame ? images.get(frame.idx) : null;
          if (!img) return;
          if (src) {
            img.src = src;
            img.style.visibility = "visible";
            if (empty) empty.style.display = "none";
          } else {
            img.removeAttribute("src");
            img.style.visibility = "hidden";
            if (empty) empty.style.display = "flex";
          }
          const range = $("#frameRange");
          if (range) range.value = String(this.pos);
          renderFrameDetail(this.pos);
          this.updateControls();
        },
        play() {
          if (total <= 1) {
            this.playing = false;
            this.updateControls();
            return;
          }
          this.playing = true;
          if (this.timer) clearInterval(this.timer);
          this.timer = setInterval(() => {
            const img = $("#animFrame");
            if (!img || !img.isConnected) {
              this.dispose();
              return;
            }
            this.renderAt((this.pos + 1) % total);
          }, 1000 / fps);
          this.updateControls();
        },
        pause() {
          if (this.timer) {
            clearInterval(this.timer);
            this.timer = null;
          }
          this.playing = false;
          this.updateControls();
        },
        toggle() {
          if (this.playing) this.pause();
          else this.play();
        },
        step(delta) {
          this.pause();
          this.renderAt(this.pos + delta);
        },
        seek(p) {
          this.pause();
          this.renderAt(p);
        },
        dispose() {
          if (this.timer) {
            clearInterval(this.timer);
            this.timer = null;
          }
          this.playing = false;
        },
      };
      player.renderAt(clampPos(startPos));
      if (startPlaying && total > 1) player.play();
      return player;
    };

    const startPreview = async ({ preserve = false } = {}) => {
      if (!previewArea.isConnected) {
        stopPreview();
        return;
      }
      const prev = preserve ? state.previewPlayer : null;
      const prevPos = prev ? prev.pos : 0;
      const prevPlaying = prev ? prev.playing : true;
      const seq = ++state.previewSeq;
      if (state.previewPlayer) {
        state.previewPlayer.dispose();
        state.previewPlayer = null;
      }
      if (!preserve) zoom = 1;
      previewArea.innerHTML = '<p class="muted">渲染中…</p>';
      $("#frameCounter").textContent = "渲染中…";
      setControlsEnabled(false);
      try {
        const res = await fetch(`/api/anim/assets/preview?${buildRenderParams("gif")}`);
        if (!res.ok) throw new Error(await res.text());
        const data = await res.json();
        if (seq !== state.previewSeq || !previewArea.isConnected) return;
        if (frames.length === 0) throw new Error("没有帧数据");
        const reports = Array.isArray(data.follows) ? data.follows : [];
        const followErrors = Array.isArray(data.follow_errors) ? data.follow_errors : [];
        state.followReports = reports;
        const followStatus = $("#followStatus");
        if (followStatus) {
          if (followErrors.length) {
            followStatus.innerHTML = `<span style="color:var(--err)">跟随 FX 未解析：${followErrors.map(e => esc(e.anchor)).join(", ")}</span>`;
          } else if (reports.length) {
            const missingFollower = reports.reduce(
              (n, r) => n + (r.follower_empty_frames || []).length,
              0,
            );
            followStatus.textContent = `跟随 FX ${reports.length} 个 · 已合成${missingFollower ? ` · ${missingFollower} 帧跟随动画缺帧` : ""}`;
          } else {
            followStatus.textContent = "";
          }
        }
        const images = new Map();
        for (const f of (data.frames || [])) {
          if (typeof f.idx === "number") images.set(f.idx, f.data);
        }
        if (images.size === 0) throw new Error("没有可预览帧");
        const fps = Math.max(1, Number(data.frame_rate) || 15);
        previewArea.innerHTML = `<img id="animFrame" style="max-width:100%;max-height:100%;width:auto;height:auto;transform-origin:center;border:1px solid var(--border);visibility:hidden;transform:scale(${zoom})">
          <div id="animEmpty" class="muted" style="position:absolute;inset:0;display:none;align-items:center;justify-content:center;pointer-events:none">本帧无可见元素</div>`;
        setControlsEnabled(true);
        const player = createPlayer({
          images,
          fps,
          startPos: prevPos,
          startPlaying: prevPlaying,
        });
        state.previewPlayer = player;
        prevBtn.onclick = () => player.step(-1);
        playBtn.onclick = () => player.toggle();
        nextBtn.onclick = () => player.step(1);
      } catch (e) {
        if (seq !== state.previewSeq || !previewArea.isConnected) return;
        setControlsEnabled(false);
        $("#frameCounter").textContent = "—";
        previewArea.innerHTML = `<p style="color:var(--err)">失败：${esc(e.message)}</p>`;
      }
    };
    startPreview();

    const schedulePreviewRefresh = () => {
      if (state.previewRefreshTimer) clearTimeout(state.previewRefreshTimer);
      state.previewRefreshTimer = setTimeout(() => {
        state.previewRefreshTimer = null;
        startPreview({ preserve: true });
      }, 250);
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


  /* ---------------- manual file source ---------------- */
  async function renderManual() {
    stopPreview();
    view().innerHTML = `
      <div class="panel">
        <h2>手动文件导入</h2>
        <div class="row">
          <input id="manualQ" placeholder="按文件名搜索，如 wilson / player_actions_axe / dynamic…" style="width:360px" value="${esc(state.manualQ)}">
          <button class="btn" id="manualSearch">搜索</button>
          <span class="muted" id="manualCount"></span>
        </div>
        <p class="muted">勾选参与渲染的归档；带 <span class="badge b-ok">anim</span> 的可设为主文件。symbol map 与候选扫描进入详情后与 prefab 线共用同一套逻辑。</p>
      </div>
      <div class="panel" id="manualResults"></div>
      <div class="panel">
        <div class="row" style="justify-content:space-between;align-items:center">
          <h2 style="margin:0">已选文件</h2>
          <button class="btn" id="manualPreview">进入预览</button>
        </div>
        <div id="manualSelected"></div>
      </div>`;
    $("#manualSearch").onclick = manualSearch;
    $("#manualQ").addEventListener("input", e => { state.manualQ = e.target.value; });
    $("#manualQ").addEventListener("keydown", e => { if (e.key === "Enter") manualSearch(); });
    $("#manualPreview").onclick = enterManualPreview;
    if (state.manualResults.length === 0) await manualSearch();
    else { renderManualResults(); renderManualSelected(); }
  }

  async function manualSearch() {
    const box = $("#manualResults");
    if (box) box.innerHTML = '<p class="muted">搜索中…</p>';
    try {
      const q = state.manualQ.trim();
      const r = await apiJSON(`/api/anim/assets/files?q=${encodeURIComponent(q)}&limit=200`);
      state.manualResults = r.files || [];
      renderManualResults();
    } catch (e) {
      if (box) box.innerHTML = `<p style="color:var(--err)">失败：${esc(e.message)}</p>`;
    }
  }

  function manualResultMeta(path) {
    return state.manualResults.find(f => f.path === path) || {};
  }

  function renderManualResults() {
    const box = $("#manualResults");
    if (!box) return;
    $("#manualCount").textContent = `共 ${state.manualResults.length} 条`;
    box.innerHTML = state.manualResults.map(f => {
      const checked = state.manualChecked.has(f.path);
      const primary = state.manualPrimary === f.path;
      return `<div class="list-row">
        <input type="checkbox" data-manual-check="${esc(f.path)}" ${checked ? "checked" : ""}>
        <code class="list-name" title="${esc(f.path)}">${esc(f.path)}</code>
        ${f.has_anim ? '<span class="badge b-ok">anim</span>' : ""}
        ${f.build_name ? `<span class="list-meta" title="build: ${esc(f.build_name)}">build: ${esc(f.build_name)}</span>` : ""}
        ${f.has_anim ? `<label class="muted" style="display:flex;align-items:center;gap:3px;margin:0;font-size:11px"><input type="radio" name="manualPrimary" value="${esc(f.path)}" ${primary ? "checked" : ""}>主文件</label>` : ""}
      </div>`;
    }).join("") || '<p class="muted">未找到文件。</p>';
    box.querySelectorAll("[data-manual-check]").forEach(chk => {
      chk.onchange = () => {
        if (chk.checked) state.manualChecked.add(chk.dataset.manualCheck);
        else {
          state.manualChecked.delete(chk.dataset.manualCheck);
          if (state.manualPrimary === chk.dataset.manualCheck) state.manualPrimary = null;
        }
        renderManualResults();
      };
    });
    box.querySelectorAll('input[name="manualPrimary"]').forEach(radio => {
      radio.onchange = () => {
        state.manualPrimary = radio.value;
        state.manualChecked.add(radio.value);
        renderManualSelected();
      };
    });
  }

  function renderManualSelected() {
    const box = $("#manualSelected");
    if (!box) return;
    const selected = Array.from(state.manualChecked);
    box.innerHTML = selected.length === 0
      ? '<p class="muted">尚未选择文件。</p>'
      : selected.map(path => {
          const meta = manualResultMeta(path);
          return `<div class="list-row">
            <span class="list-name" title="${esc(path)}"><code>${esc(path)}</code></span>
            ${state.manualPrimary === path ? '<span class="badge b-ok">主文件</span>' : (meta.has_anim ? '<span class="badge b-warn">可设主文件</span>' : "")}
            <button class="btn link" data-manual-remove="${esc(path)}">移除</button>
          </div>`;
        }).join("");
    box.querySelectorAll("[data-manual-remove]").forEach(btn => {
      btn.onclick = () => {
        const path = btn.dataset.manualRemove;
        state.manualChecked.delete(path);
        if (state.manualPrimary === path) state.manualPrimary = null;
        renderManualResults();
      };
    });
  }

  function flattenBanks(meta) {
    const banks = (meta.banks || []).map(b => b.name);
    const animations = [];
    for (const b of meta.banks || []) for (const a of b.animations || []) animations.push(a);
    return { banks, animations };
  }

  async function enterManualPreview() {
    let primary = state.manualPrimary;
    if (!primary || !state.manualChecked.has(primary)) {
      const hit = state.manualResults.find(f => state.manualChecked.has(f.path) && f.has_anim);
      primary = hit ? hit.path : null;
    }
    if (!primary) { alert("请勾选并指定一个含 anim 的主文件"); return; }
    state.manualPrimary = primary;
    const meta = manualResultMeta(primary);
    const firstBank = (meta.banks || [])[0] || {};
    const bank = firstBank.name;
    const anim = (firstBank.animations || [])[0];
    if (!bank || !anim) { alert("主文件没有可预览的 bank/animation"); return; }
    state.source = "manual";
    state.manualFiles = Array.from(state.manualChecked);
    if (!state.manualFiles.includes(primary)) state.manualFiles.push(primary);

    state.animFiles = [];
    state.relatedFiles = [];
    state.fileInfo = [];
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
    state.prefabItems = [];
    state.follows = {};
    state.followTouched = new Set();
    state.anchorSymbols = new Set();
    state.followCandidates = {};
    state.followReports = [];
    for (const path of state.manualFiles) {
      const flat = flattenBanks(manualResultMeta(path));
      state.fileInfo.push({ path, content: flat });
      if ((flat.banks || []).length || (flat.animations || []).length) state.animFiles.push(path);
      else state.relatedFiles.push(path);
    }
    if (!state.animFiles.includes(primary)) state.animFiles.unshift(primary);
    await openAnimation(primary, bank, anim);
  }

  // Initial load
  view().innerHTML = '<div class="panel"><p class="muted">加载中…</p></div>';
  await search();
}

/* ---------------- animation diff ---------------- */
