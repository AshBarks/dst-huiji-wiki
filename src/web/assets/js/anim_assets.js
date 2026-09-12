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
    previewTimer: null,
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
    renderAnimationDetail();
  }

  function renderAnimationDetail() {
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

    $("#backToPrefab").onclick = () => (state.source === "manual" ? renderManual() : renderList());

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
            const via = b.via === "symbol_map"
              ? '<span class="badge b-ok">map</span>'
              : b.via === "both"
                ? '<span class="badge b-ok">map+同名</span>'
                : "";
            const matched = (b.matched_symbols || []).join(",")
              || (b.remaps || []).map(r => `${r.symbol}→${r.src_symbol}`).join(",");
            return `<div class="list-row">
              <input type="checkbox" data-candidate-toggle="${esc(b.file)}" ${checked ? "checked" : ""} ${disabled ? "disabled" : ""}>
              <code class="list-name" title="${esc(b.file)}">${esc(b.file)}</code>
              ${via}
              <span class="list-meta" title="${esc(matched)}">${esc(matched)}</span>
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
                <code class="list-name" style="flex:0 1 auto" title="${esc(sym)}">${esc(sym)}</code>${hidden ? '<span class="badge b-warn">hidden</span>' : ""}${autoIdx >= 0 ? '<span class="badge b-ok">auto-map</span>' : ""}
                <button class="btn link" data-sym-scan="${esc(sym)}" title="扫描含该 symbol（同名 ∪ symbol map）的可用 build">扫描 build</button>
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


  /* ---------------- manual file source ---------------- */
  async function renderManual() {
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
