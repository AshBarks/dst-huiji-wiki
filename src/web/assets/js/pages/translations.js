//! 翻译页
"use strict";

import { esc, api, getJSON, pager, META, $ } from "../util.js";

export async function pageTranslations(main) {
  main.innerHTML = `
    <div class="panel"><h2>分类进度（点击查看条目）</h2><div id="tchart"></div></div>
    <div class="panel">
      <h2 id="ttitle">全部条目</h2>
      <div class="row">
        <input id="tq" placeholder="搜索…" style="width:220px">
        <select id="ttf"><option value="">全部</option><option value="yes">已翻译</option><option value="no">未翻译</option></select>
      </div>
      <div id="ttable" style="margin-top:10px"></div><div id="tpager"></div>
    </div>`;

  const cats = META.po_categories.filter(c => c.category !== "NONE").slice(0, 18);
  const maxTotal = Math.max(...cats.map(c => c.total), 1);
  const W = 1060, rowH = 26, barW = W - 320;
  const svgParts = cats.map((c, i) => {
    const y = i * rowH + 14;
    const w1 = (c.total / maxTotal) * barW;
    const w2 = (c.translated / maxTotal) * barW;
    const pct = c.total ? Math.round(100 * c.translated / c.total) : 0;
    return `<g class="bar-wrap" data-cat="${esc(c.category)}" style="cursor:pointer">
      <text x="0" y="${y + 4}" fill="var(--muted)" font-size="12">${esc(c.category)}</text>
      <rect x="150" y="${y - 8}" width="${w1}" height="16" rx="4" fill="#2c3547"/>
      <rect x="150" y="${y - 8}" width="${w2}" height="16" rx="4" fill="#3fb96e"/>
      <text x="${150 + w1 + 8}" y="${y + 4}" fill="var(--muted)" font-size="11">${pct}% (${c.translated}/${c.total})</text>
    </g>`;
  }).join("");
  $("#tchart").innerHTML = `<svg viewBox="0 0 ${W} ${cats.length * rowH + 20}" width="100%">${svgParts}</svg>`;

  const st = { cat: null, q: "", tf: "", page: 0, size: 50 };
  const loadEntries = async () => {
    const p = new URLSearchParams({ category: st.cat || "", q: st.q, translated: st.tf,
      page: st.page, page_size: st.size });
    const r = await getJSON(`/api/data/po/entries?${p}`);
    $("#ttable").innerHTML = `<table><thead><tr><th>上下文 msgctxt</th><th>英文 msgid</th><th>中文 msgstr</th></tr></thead><tbody>
      ${r.items.map(e => `<tr>
        <td class="muted"><small>${esc(e.msgctxt || "")}</small></td>
        <td>${esc(e.msgid)}</td>
        <td style="${e.msgstr.trim() ? "" : "color:var(--err)"}">${esc(e.msgstr) || "（空）"}</td>
      </tr>`).join("")}</tbody></table>`;
    $("#tpager").innerHTML = "";
    $("#tpager").appendChild(pager(r.total, r.page, r.page_size, (pg) => { st.page = pg; loadEntries(); }));
  };

  document.querySelectorAll("#tchart g.bar-wrap").forEach(g => g.onclick = async () => {
    st.cat = g.dataset.cat; st.page = 0;
    $("#ttitle").textContent = `分类：${st.cat}`;
    await loadEntries();
    document.querySelector("#ttitle").scrollIntoView({ behavior: "smooth" });
  });
  let deb;
  $("#tq").oninput = (e) => { clearTimeout(deb); deb = setTimeout(() => { st.q = e.target.value.trim(); st.page = 0; loadEntries(); }, 250); };
  $("#ttf").onchange = (e) => { st.tf = e.target.value; st.page = 0; loadEntries(); };
  await loadEntries();
}

/* ---------------- skill trees ---------------- */
