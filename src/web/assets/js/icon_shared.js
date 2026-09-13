//! 图标页共享件（物品栏 / 制作栏两页通用）：状态标签、徽章、
//! 详情弹窗（历代版本 + 上传/映射编辑，接口按 file 寻址、来源无关）。
"use strict";

import { esc, getJSON, postJSON, $ } from "./util.js";

export const STATUS_LABELS = {
  uploaded: "已上传", missing: "未上传", unknown: "状态未知",
  unuploadable: "无法上传", unnamed: "无英文名",
};

export const fmtDate = (ms) => (ms ? new Date(ms).toLocaleString() : "");

export function iconBadge(it) {
  if (!it.status) return "";
  if (it.status === "uploaded") return '<span class="badge b-success">已上传</span>';
  if (it.status === "missing") return '<span class="badge b-err">未上传</span>';
  if (it.status === "unuploadable") return `<span class="badge b-warn" title="${esc(it.note || "")}">无法上传</span>`;
  if (it.status === "unknown") return '<span class="badge b-muted">状态未知</span>';
  return `<span class="badge b-muted">${esc(STATUS_LABELS[it.status] || it.status)}</span>`;
}

/// 注入详情弹窗骨架并绑定关闭行为；返回打开详情的函数 `openDetail(file)`。
/// `onRefresh` 在保存/清除映射后调用（调用方刷新自己的列表）。
export function mountIconDetail(main, onRefresh) {
  main.insertAdjacentHTML("beforeend", `
    <div id="ivOverlay" class="iv-overlay" style="display:none">
      <div class="iv-panel">
        <div class="row" style="justify-content:space-between">
          <h2 id="ivTitle" style="margin:0"></h2>
          <button class="btn secondary" id="ivClose">关闭</button>
        </div>
        <div id="ivList"></div>
      </div>
    </div>`);

  const overlay = $("#ivOverlay");
  $("#ivClose").onclick = () => { overlay.style.display = "none"; };
  overlay.onclick = (e) => { if (e.target === overlay) overlay.style.display = "none"; };

  const openDetail = async (file) => {
    $("#ivTitle").textContent = file;
    $("#ivList").innerHTML = '<p class="muted">加载中…</p>';
    overlay.style.display = "flex";
    try {
      const r = await getJSON(`/api/data/inventoryicons/versions?file=${encodeURIComponent(file)}`);
      const names = [r.name_zh, r.name_en].filter(Boolean).join(" / ");
      const exists = !!(r.wiki && r.wiki.exists === true);
      const badge = iconBadge(r);
      const source = r.title_source === "override" ? "映射" : r.title_source === "auto" ? "自动" : "无";
      const target = r.wiki_title
        ? `<div class="muted">当前生效：<code>${esc(r.wiki_title)}</code>（${source}）</div>`
        : "";
      const link = r.wiki && r.wiki.url
        ? `<div class="muted" style="word-break:break-all">当前文件：<a href="${esc(r.wiki.url)}" target="_blank" rel="noopener">${esc(r.wiki.url)}</a></div>`
        : "";
      const editor = `
        <div class="iv-editor">
          <input id="ivName" value="${esc(r.title || "")}" placeholder="维基文件名，如 Pick-Axe.png">
          <div class="row" style="gap:6px;flex-wrap:wrap">
            <button class="btn" id="ivUpload"${r.present ? "" : " disabled"}>${exists ? "重新上传（覆盖）" : "上传到维基"}</button>
            <button class="btn secondary" id="ivSaveMap">保存映射</button>
            ${r.title_source === "override" ? '<button class="btn secondary" id="ivClearMap">恢复自动命名</button>' : ""}
          </div>
          ${r.present ? "" : '<div class="muted">该图标不在当前 build 产物中，无法上传。</div>'}
        </div>`;
      const head = `
        <div class="iv-actions">
          <div>${names ? `<b>${esc(names)}</b>` : '<span class="muted">暂无游戏内名称</span>'} ${badge}</div>
          ${target}
          ${link}
          ${editor}
        </div>`;
      $("#ivList").innerHTML = head + (r.versions.map((v, i) => `
        <div class="icon-ver">
          <img loading="lazy" width="64" height="64" src="/static/objects/${esc(v.hash)}" alt="">
          <div>
            <b>Build ${esc(v.build)}</b>${v.synced_at ? `<span class="muted"> · ${fmtDate(v.synced_at)}</span>` : ""}
            ${i === 0 && r.present ? ' <span class="badge b-success">当前</span>' : ""}
          </div>
          <code class="muted" style="margin-left:auto">${esc(v.hash.slice(0, 12))}…</code>
        </div>`).join("") || '<p class="muted">无历史记录。</p>');

      const readTitle = () => ($("#ivName").value || "").trim();
      const ub = $("#ivUpload");
      if (ub) ub.onclick = async () => {
        const title = readTitle();
        if (!title) { alert("请先填写维基文件名"); return; }
        const msg = `将上传 ${file} 到 File:${title}${exists ? "（同名已存在则覆盖重传）" : ""}；标题与自动命名不同时会一并写入映射表。确定继续？`;
        if (!confirm(msg)) return;
        ub.disabled = true;
        try {
          const j = await postJSON("/api/jobs", {
            kind: "upload_icons", file, title, wiki_dry_run: false,
          });
          location.hash = `#/jobs/${j.id}`;
        } catch (err) { ub.disabled = false; alert("提交失败：" + err.message); }
      };
      const saveBtn = $("#ivSaveMap");
      if (saveBtn) saveBtn.onclick = async () => {
        const title = readTitle();
        if (!title) { alert("请先填写维基文件名"); return; }
        saveBtn.disabled = true;
        try {
          await postJSON("/api/data/inventoryicons/title", { file, title });
          onRefresh();
          await openDetail(file);
        } catch (err) { saveBtn.disabled = false; alert("保存失败：" + err.message); }
      };
      const clearBtn = $("#ivClearMap");
      if (clearBtn) clearBtn.onclick = async () => {
        if (!confirm(`清除 ${file} 的映射并恢复自动命名？`)) return;
        clearBtn.disabled = true;
        try {
          await postJSON("/api/data/inventoryicons/title", { file, title: null });
          onRefresh();
          await openDetail(file);
        } catch (err) { clearBtn.disabled = false; alert("清除失败：" + err.message); }
      };
    } catch (err) {
      $("#ivList").innerHTML = `<p style="color:var(--err)">加载失败：${esc(err.message)}</p>`;
    }
  };

  return openDetail;
}
