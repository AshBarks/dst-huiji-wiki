//! 共享工具与全局状态（ES module，无打包器）。
"use strict";

export const $ = (s) => document.querySelector(s);
export const esc = (s) => String(s ?? "").replace(/[&<>"']/g,
  c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));

export let META = null;            // /api/data/meta for currentSnapshot
export let currentSnapshot = "";   // "" = latest
/// 路由级定时器（任务详情页自动刷新用），跨模块共享。
export const routeTimerRef = { id: null };

export async function api(path, opts) {
  const res = await fetch(path, opts);
  if (!res.ok) {
    let detail = res.statusText;
    try { const j = await res.json(); if (j.error) detail = j.error; } catch {}
    throw new Error(`${res.status}: ${detail}`);
  }
  return res.json();
}
export const getJSON = (p) => api(p);
export const postJSON = (p, body) => api(p, {
  method: "POST", headers: { "Content-Type": "application/json" },
  body: JSON.stringify(body || {})
});


export function fmtTime(ms) {
  if (!ms) return "—";
  return new Date(ms).toLocaleTimeString();
}


export async function loadMeta() {
  META = await getJSON(`/api/data/meta?snapshot=${encodeURIComponent(currentSnapshot)}`);
}

/* ---------------- shared widgets ---------------- */
export function statusBadge(status) {
  return `<span class="badge b-${esc(status)}">${esc(status)}</span>`;
}

export function elFromHtml(html) {
  const t = document.createElement("template");
  t.innerHTML = html.trim();
  return t.content.firstElementChild;
}

export function diffCardHtml(d) {
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

export function snapshotSelect(onChangeId) {
  return `<select id="${onChangeId}" title="数据版本">
    <option value="">最新</option>
  </select>`;
}
export async function fillSnapshots(selectEl) {
  const r = await getJSON("/api/snapshots");
  for (const s of r.snapshots) {
    const opt = document.createElement("option");
    opt.value = s.name; opt.textContent = s.name;
    selectEl.appendChild(opt);
  }
  selectEl.value = currentSnapshot;
  selectEl.addEventListener("change", () => {
    currentSnapshot = selectEl.value;
    // 通知路由层重渲染（避免 util → app 的循环导入）。
    window.dispatchEvent(new Event("dstui:rerender"));
  });
}

export function pager(total, page, pageSize, onGo) {
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
