//! 配方/材料页
"use strict";

import { esc, api, getJSON, pager, META, $ } from "../util.js";

export async function pageRecipes(main) {
  main.innerHTML = `
    <div class="panel">
      <div class="row">
        <input id="rq" placeholder="搜索配方名…" style="width:220px">
        <select id="rtech"><option value="">全部科技</option></select>
        <input id="ring" placeholder="材料反查（点击表格中的材料也可）…" style="width:260px">
        <button class="btn secondary" id="rclear">清空筛选</button>
        <span class="muted" id="rcount"></span>
      </div>
    </div>
    <div class="panel"><div id="rtable"></div><div id="rpager"></div></div>`;

  for (const t of META.tech_levels) {
    const o = document.createElement("option"); o.textContent = t; $("#rtech").appendChild(o);
  }

  const state = { q: "", tech: "", ing: "", page: 0, size: 60 };
  const load = async () => {
    const p = new URLSearchParams({ q: state.q, tech: state.tech, ingredient: state.ing,
      page: state.page, page_size: state.size });
    const r = await getJSON(`/api/data/recipes?${p}`);
    $("#rcount").textContent = `共 ${r.total} 个配方`;
    $("#rtable").innerHTML = `<table><thead><tr><th>名称</th><th>科技</th><th>材料</th></tr></thead><tbody>
      ${r.items.map(x => `<tr>
        <td><code>${esc(x.name)}</code></td><td>${esc(x.tech)}</td>
        <td>${x.ingredients.map(i =>
          `<span class="chip" data-ing="${esc(i.item)}">${esc(i.item)} ×${i.amount}</span>`).join("")}
        </td></tr>`).join("")}</tbody></table>`;
    $("#rpager").innerHTML = "";
    $("#rpager").appendChild(pager(r.total, r.page, r.page_size, (pg) => { state.page = pg; load(); }));
    document.querySelectorAll("#rtable .chip").forEach(c =>
      c.onclick = () => { state.ing = c.dataset.ing; $("#ring").value = state.ing; state.page = 0; load(); });
  };

  let deb;
  $("#rq").oninput = (e) => { clearTimeout(deb); deb = setTimeout(() => { state.q = e.target.value.trim(); state.page = 0; load(); }, 250); };
  $("#ring").oninput = (e) => { clearTimeout(deb); deb = setTimeout(() => { state.ing = e.target.value.trim(); state.page = 0; load(); }, 250); };
  $("#rtech").onchange = (e) => { state.tech = e.target.value; state.page = 0; load(); };
  $("#rclear").onclick = () => { state.q = state.tech = state.ing = ""; $("#rq").value = $("#ring").value = ""; $("#rtech").value = ""; state.page = 0; load(); };
  await load();
}

/* ---------------- translations ---------------- */
