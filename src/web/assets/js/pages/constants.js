//! 常量页
"use strict";

import { esc, api, getJSON, pager, $ } from "../util.js";

export async function pageConstants(main) {
  main.innerHTML = `
    <div class="panel">
      <div class="row">
        <input id="cq" placeholder="搜索 TUNING 键或值…" style="width:280px">
        <span class="muted" id="ccount"></span>
      </div>
    </div>
    <div class="panel"><div id="ctable"></div><div id="cpager"></div></div>`;
  const st = { q: "", page: 0, size: 80 };
  const load = async () => {
    const p = new URLSearchParams({ q: st.q, page: st.page, page_size: st.size });
    const r = await getJSON(`/api/data/constants?${p}`);
    $("#ccount").textContent = `共 ${r.total} 个常量`;
    $("#ctable").innerHTML = `<table><thead><tr><th style="width:340px">键</th><th>值</th></tr></thead><tbody>
      ${r.items.map(x => `<tr><td><code>${esc(x.key)}</code></td><td>${esc(x.value)}</td></tr>`).join("")}
      </tbody></table>`;
    $("#cpager").innerHTML = "";
    $("#cpager").appendChild(pager(r.total, r.page, r.page_size, (pg) => { st.page = pg; load(); }));
  };
  let deb;
  $("#cq").oninput = (e) => { clearTimeout(deb); deb = setTimeout(() => { st.q = e.target.value.trim(); st.page = 0; load(); }, 250); };
  await load();
}

/* ---------------- animation assets ---------------- */
