//! 烹饪模拟页：后端只提供编译好的 JSON，候选匹配与随机都在浏览器完成。
"use strict";

import {
  $, esc, getJSON, currentSnapshot, snapshotSelect, fillSnapshots,
} from "../util.js";
import {
  SCHEMA_VERSION, evaluateRecipes, pickUniform,
} from "../cooking_eval.js";

export async function pageCooking(main) {
  main.innerHTML = '<div class="panel"><p class="muted">正在加载烹饪数据…</p></div>';
  const data = await getJSON(
    `/api/data/cooking?snapshot=${encodeURIComponent(currentSnapshot)}`,
  );
  if (data.schema_version !== SCHEMA_VERSION) {
    throw new Error(`烹饪数据 schema ${data.schema_version} 与前端 ${SCHEMA_VERSION} 不匹配`);
  }

  main.innerHTML = `
    <div class="panel">
      <div class="row">
        <h2 style="margin:0">烹饪模拟</h2>
        <select id="cookType">
          <option value="cookpot">烹饪锅</option>
          <option value="portablecookpot">便携烹饪锅</option>
        </select>
        <input id="cookSearch" placeholder="搜索食材 / 标签…" style="width:220px">
        <span class="muted" id="cookMeta"></span>
        <span style="flex:1"></span>
        ${snapshotSelect("cookSnap")}
      </div>
    </div>
    <div class="cook-layout">
      <div class="panel">
        <h2>食材 <span class="muted" id="cookIngCount"></span></h2>
        <div id="cookGrid" class="cook-grid"></div>
      </div>
      <div>
        <div class="panel">
          <h2>已选食材 <span class="muted">4 格，可重复</span></h2>
          <div id="cookSlots" class="cook-slots"></div>
          <div class="row" style="margin-top:12px">
            <button class="btn secondary" id="cookClear">清空</button>
            <button class="btn" id="cookGo" disabled>烹饪</button>
            <span class="muted" id="cookHint"></span>
          </div>
        </div>
        <div class="panel" id="cookResult"></div>
      </div>
    </div>`;

  const state = {
    cooker: "cookpot",
    selected: [],
    query: "",
    last: null,
    chosenIndex: -1,
    hint: "",
  };

  $("#cookMeta").textContent =
    `${data.ingredients.length} 种真实食材 · ${Object.keys(data.recipes).length} 个食谱 · ${data.label}`;

  await fillSnapshots($("#cookSnap"));

  const labelOf = (entry) =>
    entry.name_zh || entry.name_en || entry.prefab || entry.name;

  const iconHtml = (entry, size = 40) => {
    if (!entry || !entry.icon) {
      return `<span class="cook-noicon" style="width:${size}px;height:${size}px">?</span>`;
    }
    return `<img loading="lazy" width="${size}" height="${size}"
      src="/static/split/inventoryimages/${encodeURIComponent(entry.icon)}"
      alt="${esc(entry.prefab || entry.name || "")}">`;
  };

  const tagText = (entry) =>
    Object.entries(entry.tags || {})
      .map(([key, value]) => `${key} ${Number(value).toFixed(2).replace(/\.00$/, "")}`)
      .join(" · ");

  const renderPalette = () => {
    const q = state.query.trim().toLowerCase();
    const items = data.ingredients.filter((entry) => {
      if (!q) return true;
      const haystack = [
        entry.prefab, entry.key, entry.name_zh, entry.name_en,
        ...Object.keys(entry.tags || {}),
      ].filter(Boolean).join(" ").toLowerCase();
      return haystack.includes(q);
    });
    $("#cookIngCount").textContent = `（显示 ${items.length} / 共 ${data.ingredients.length}）`;
    $("#cookGrid").innerHTML = items.map((entry) => `
      <figure class="cook-ing" data-prefab="${esc(entry.prefab)}"
        title="${esc(tagText(entry) || entry.prefab)}">
        ${iconHtml(entry)}
        <figcaption>
          <b>${esc(labelOf(entry))}</b>
          <code>${esc(entry.prefab)}</code>
        </figcaption>
      </figure>`).join("");

    $("#cookGrid").querySelectorAll(".cook-ing").forEach((card) => {
      card.onclick = () => {
        const entry = data.ingredients.find((i) => i.prefab === card.dataset.prefab);
        if (!entry) return;
        if (state.selected.length >= 4) {
          state.hint = "已选满 4 格，点选槽位可移除后再添加";
          state.last = null;
          renderSlots();
          renderResult();
          return;
        }
        state.selected.push(entry);
        state.hint = "";
        state.last = null;
        renderSlots();
        renderResult();
      };
    });
  };

  const renderSlots = () => {
    $("#cookSlots").innerHTML = Array.from({ length: 4 }, (_, index) => {
      const entry = state.selected[index];
      if (!entry) {
        return `<div class="cook-slot empty" data-slot="${index}">空</div>`;
      }
      return `
        <div class="cook-slot" data-slot="${index}" title="点击移除 ${esc(entry.prefab)}">
          ${iconHtml(entry, 36)}
          <span>${esc(labelOf(entry))}</span>
        </div>`;
    }).join("");

    $("#cookSlots").querySelectorAll(".cook-slot").forEach((slot) => {
      slot.onclick = () => {
        const index = Number(slot.dataset.slot);
        state.selected.splice(index, 1);
        state.hint = "";
        state.last = null;
        renderSlots();
        renderResult();
      };
    });

    $("#cookGo").disabled = state.selected.length !== 4;
    $("#cookHint").textContent = state.hint;
  };

  const renderResult = () => {
    const resultEl = $("#cookResult");
    if (!state.last) {
      resultEl.innerHTML = '<p class="muted">选择 4 个食材后点击“烹饪”：会按最高优先级等概率随机产出，并列出所有同优先级可能。</p>';
      return;
    }
    const { matches, top, chosenIndex } = state.last;
    const chosen = top[chosenIndex];
    const probability = top.length > 1 ? `${((1 / top.length) * 100).toFixed(1)}%` : "确定";
    const otherMatches = matches.length - top.length;

    resultEl.innerHTML = `
      <h2>烹饪结果</h2>
      <div class="cook-result-main">
        ${iconHtml(chosen, 56)}
        <div>
          <div class="muted">本次产出</div>
          <h2 style="margin:0">${esc(labelOf(chosen))}</h2>
          <code>${esc(chosen.name)}</code>
          <span class="badge b-success">优先级 ${esc(chosen.priority)}</span>
          ${top.length > 1 ? `<span class="badge b-warn">概率 ${probability}</span>` : ""}
        </div>
      </div>
      <h3>同优先级候选 <span class="muted">${top.length} 个，等概率；加粗为本次结果</span></h3>
      <div class="cook-candidates">
        ${top.map((recipe, index) => `
          <div class="cook-candidate ${index === chosenIndex ? "selected" : ""}">
            ${iconHtml(recipe, 44)}
            <div>
              <b>${esc(labelOf(recipe))}</b>
              <code>${esc(recipe.name)}</code>
              <div class="muted">${top.length > 1 ? `概率 ${probability}` : "唯一最高优先级"}</div>
            </div>
            ${index === chosenIndex ? '<span class="badge b-success">本次</span>' : ""}
          </div>`).join("")}
      </div>
      ${otherMatches > 0 ? `<p class="muted">另有 ${otherMatches} 个食谱满足条件，但优先级低于最高档，不会产出。</p>` : ""}
    `;
  };

  const roll = () => {
    if (state.selected.length !== 4) return;
    try {
      const evaluated = evaluateRecipes(data, state.cooker, state.selected);
      if (!evaluated.top.length) {
        state.last = null;
        $("#cookResult").innerHTML =
          '<p style="color:var(--err)">没有匹配到任何食谱（数据可能不完整，请检查游戏脚本版本）。</p>';
        return;
      }
      const chosenIndex = pickUniform(evaluated.top);
      state.last = { ...evaluated, chosenIndex };
      state.hint = "";
      renderSlots();
      renderResult();
    } catch (err) {
      state.last = null;
      $("#cookResult").innerHTML =
        `<p style="color:var(--err)">计算失败：${esc(err.message)}</p>`;
    }
  };

  $("#cookType").onchange = (event) => {
    state.cooker = event.target.value;
    state.last = null;
    state.hint = "";
    renderSlots();
    renderResult();
  };

  let debounceTimer = null;
  $("#cookSearch").oninput = (event) => {
    clearTimeout(debounceTimer);
    debounceTimer = setTimeout(() => {
      state.query = event.target.value;
      renderPalette();
    }, 150);
  };

  $("#cookClear").onclick = () => {
    state.selected = [];
    state.last = null;
    state.hint = "";
    renderSlots();
    renderResult();
  };
  $("#cookGo").onclick = roll;

  renderPalette();
  renderSlots();
  renderResult();

}
