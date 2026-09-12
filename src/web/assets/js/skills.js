//! 技能树页（与零件:Skilltree.js 的人工同步部分）
"use strict";

import { esc, api, getJSON, META, $ } from "./util.js";

export async function pageSkills(main) {
  const chars = META.skill_characters || [];
  main.innerHTML = `
    <div class="panel">
      <div class="row">
        <label style="margin:0">角色</label>
        <select id="charSel">${chars.map(c => `<option>${esc(c)}</option>`).join("")}</select>
        <span class="muted">布局与交互 1:1 还原游戏内界面（参考零件:Skilltree.js）。</span>
        <span class="muted" id="skillXp"></span>
      </div>
      <p class="muted" style="margin:6px 0 0">点击技能/锁查看详情；可选技能可通过“学习”按钮或空格键点亮；双击背景或“重置洞察”清空已学技能；lock 节点按学习情况与外部条件自动解锁。</p>
    </div>
    <div class="panel"><div id="skwrap"></div>
      <div class="skilltree-info" id="skInfo">
        <b id="skTitle"></b>
        <p id="skDesc" class="muted"></p>
      </div>
    </div>`;

  const sel = $("#charSel");
  sel.value = chars.includes("wilson") ? "wilson" : chars[0];

  // 坐标体系与游戏 widgets/redux/skilltreewidget.lua 与 skilltreebuilder.lua
  // 一致：SVG 视口 = 部件根坐标（向右 +x，游戏 y 向上 → 视口 y 取负）。
  //   - 弹窗背景 background.tex 600x460 @ (0,-20)（playerinfopopupscreen.MakeBG）
  //   - 角色背景 625x384 原图按 521x320 绘制 @ (5,50)（skilltreewidget.lua）
  //   - 技能树 root 在 (0,-50)，buildbuttons 再偏移 -30 ⇒ 节点画面位置 (x, y-80)
  //   - 洞察面板 root.xp = (3,215)，同样受 root 偏移影响 ⇒ 画面 (3,165)
  const WIDTH = 600;
  const SVG_HEIGHT = 460;
  const VIEW_TOP = -210;                        // 游戏 y=210（背景上沿）→ 视口顶部
  const BG_ART_W = 521, BG_ART_H = 320;         // BG_WIDTH_INGAME / BG_HEIGHT_INGAME
  const BG_X = 5 - BG_ART_W / 2;
  const BG_Y = -(50 + BG_ART_H / 2);
  const NODE_Y_OFFSET = -80;                    // panel -30 + tree -50
  const ICON_SIZE = 28;                         // TILESIZE-4
  const INFO_ICON_SIZE = 27;                    // TILESIZE_INFOGRAPHIC
  const ICON_BUTTON_SIZE = 32;                  // TILESIZE
  const INFO_BUTTON_SIZE = (ICON_BUTTON_SIZE - 5) * Math.SQRT2;
  const INFO_FRAME_SIZE = INFO_BUTTON_SIZE * (80 / 64) - 4;
  const LOCK_SIZE = ICON_BUTTON_SIZE * 0.8;     // 锁按钮 SetScale(0.8)
  const SKILL_FOCUS_SIZE = 40;                  // TILESIZE_FRAME
  const LOCK_FOCUS_SIZE = 40;
  const TOTAL_XP = 15;
  const XP_SIZE = 50;
  const XP_X = 3;
  const XP_Y = 215 - 50;                        // root.xp (3,215) + tree -50
  const buttonWidth = 180;
  const buttonHeight = 43;
  const buttonLeftX = -130 - buttonWidth / 2;
  const buttonRightX = 130 - buttonWidth / 2;
  const buttonY = 150;                          // 游戏坐标 y=-150（背景下沿之下）

  const skillAsset = (name) => `/static/split/skilltree/${encodeURIComponent(name)}.png`;
  const reduxAsset = (name) => `/static/split/global_redux/${encodeURIComponent(name)}.png`;
  const iconUrl = (icon) => `/static/split/skilltree_icons/${encodeURIComponent(icon)}.png`;

  let tree = null;
  let nodeByName = {}; // name -> node
  let skills = {};     // name -> node（icon 存在的节点；含信息板）
  let locks = {};      // name -> node（lock_open 节点；含信息板锁）
  let parents = {};    // skill -> 直接父技能（connects 指向它的技能）

  let activatedSkills = new Set();
  let focusing = null;
  // 每个节点的渲染引用
  let gfx = {};      // name -> {bg, icon, isLock, infographic, x, y}
  let decoGfx = {};  // name -> [decoration <image>]
  let skillFocusEle = null;
  let lockFocusEle = null;
  let learnBtn = null;       // {normal, hover, down, learned, text}
  let resetBtn = null;

  // 节点画面位置 = (x, y + NODE_Y_OFFSET)，再翻转到 SVG 视口。
  const px = (x) => x;
  const py = (y) => -(y + NODE_Y_OFFSET);

  // API 对所有非锁节点也序列化 lock_open:null，必须显式排除 null。
  const isLockNode = (n) => !!n && n.lock_open !== undefined && n.lock_open !== null;

  // 可学习的普通技能（锁与信息板都不吃洞察点，与游戏 rpc_id 规则一致）。
  const learnable = (name) => {
    const n = skills[name];
    return !!n && !isLockNode(n) && !n.infographic;
  };

  function buildMaps(nodes) {
    nodeByName = {}; skills = {}; locks = {}; parents = {};
    for (const n of nodes) {
      nodeByName[n.name] = n;
      if (n.icon) skills[n.name] = n;
      if (isLockNode(n)) locks[n.name] = n;
    }
    for (const n of nodes) {
      if (learnable(n.name)) parents[n.name] = [];
    }
    for (const n of nodes) for (const c of (n.connects || [])) {
      if (learnable(n.name) && parents[c]) parents[c].push(n.name);
      // 与零件:Skilltree.js 相同：锁的 connects 子技能把该锁并入自己的
      // locks（"只有一个lock的skill会没有locks"），进入 must_have_all_of。
      if (locks[n.name] && skills[c]) {
        const child = skills[c];
        child.locks = child.locks || [];
        if (!child.locks.includes(n.name)) child.locks.push(n.name);
      }
    }
  }

  const remainingXp = () => TOTAL_XP - activatedSkills.size;

  function countTags(tag) {
    let count = 0;
    for (const name of activatedSkills) {
      const skill = skills[name];
      if (skill && skill.tags && skill.tags.includes(tag)) count += 1;
    }
    return count;
  }

  // lock_open 声明式条件求值（与零件:Skilltree.js 的 checkLockOpen 一致）。
  function evalLockCond(cond) {
    if (cond === true || cond === false || typeof cond === "number" || typeof cond === "string") return cond;
    if (typeof cond !== "object" || cond === null) return false;
    if (cond.Achievement) return true; // 外部成就类条件在本地默认视为已解锁。
    for (const key in cond) {
      const val = cond[key];
      switch (key) {
        case "GreaterThan": return evalLockCond(val.left) > evalLockCond(val.right);
        case "GreaterOrEqThan": return evalLockCond(val.left) >= evalLockCond(val.right);
        case "LessThan": return evalLockCond(val.left) < evalLockCond(val.right);
        case "LessOrEqThan": return evalLockCond(val.left) <= evalLockCond(val.right);
        case "Eq": return evalLockCond(val.left) === evalLockCond(val.right);
        case "And": return evalLockCond(val.left) && evalLockCond(val.right);
        case "Or": return evalLockCond(val.left) || evalLockCond(val.right);
        case "Not": return !evalLockCond(val);
        case "CountTags": return countTags(val);
        case "CountSkills": return activatedSkills.size;
        case "ActivatedSkill": return activatedSkills.has(val);
        case "Add": return evalLockCond(val.left) + evalLockCond(val.right);
        case "Inclination": {
          const state = inclinationState(val);
          return state ? state.side : null;
        }
        default: return false;
      }
    }
    return false;
  }

  // 沃拓克斯天秤：复刻 skilltree_wortox.lua 的 CUSTOM_FUNCTIONS.CalculateInclination。
  // affinity 生效时先向所选阵营修正一档，再做阈值比较。
  // 返回 null 或 { nice, naughty, diff, side, threshold, affinity }。
  function inclinationState(inc) {
    if (!inc || typeof inc !== "object") return null;
    const nice = Number(evalLockCond(inc.nice)) || 0;
    const naughty = Number(evalLockCond(inc.naughty)) || 0;
    let diff = nice - naughty;
    const affinity = evalLockCond(inc.affinity);
    if (affinity) {
      if (diff < 0) diff -= 1;
      else if (diff > 0) diff += 1;
    }
    const threshold = Number(inc.threshold) || 0;
    const side = threshold > 0 && Math.abs(diff) >= threshold
      ? (diff > 0 ? "nice" : "naughty")
      : null;
    return { nice, naughty, diff, side, threshold, affinity: affinity || null };
  }

  // 从任一天秤锁节点取回 Inclination 条件（nice/naughty 两侧共享参数）。
  function findInclination() {
    for (const n of tree.nodes) {
      const inc = n.lock_open
        && n.lock_open.Eq
        && n.lock_open.Eq.left
        && n.lock_open.Eq.left.Inclination;
      if (inc) return inc;
    }
    return null;
  }

  function isLockOpen(name) {
    const lock = locks[name];
    // 未显式给出条件时默认视为已解锁：外部成就等条件本地无法验证。
    if (!lock || lock.lock_open === undefined || lock.lock_open === null) return true;
    return !!evalLockCond(lock.lock_open);
  }

  // 节点底图（与游戏 RefreshTree 的分支顺序一致）：
  //   锁 → 信息板锁 on/off，普通锁 unlocked/locked
  //   信息板 → infographic
  //   普通技能 → selected/selectable/unselected
  function baseTexture(name) {
    const n = nodeByName[name];
    if (!n) return "unselected";
    if (isLockNode(n)) {
      const open = isLockOpen(name);
      if (n.infographic) return open ? "infographic_on" : "infographic_off";
      return open ? "unlocked" : "locked";
    }
    if (n.infographic) return "infographic";
    return statusOf(name) || "unselected";
  }

  // 可学条件与零件:Skilltree.js / 游戏激活校验（ValidateCharacterData 的
  // must_have_one_of / must_have_all_of）一致：
  //   root，或（所有 locks 打开 且 有父技能被激活——无父技能视为可达）。
  // 注意：skilltreebuilder 的显示循环虽然对带 locks 的技能只检查锁，
  // 但真正学习要过服务端校验，父技能激活（或父为已开锁）仍是前提。
  function statusOf(name) {
    const skill = skills[name];
    if (!skill || !learnable(name)) return null;
    if (activatedSkills.has(name)) return "selected";
    if (remainingXp() <= 0) return "unselected";
    const unlocked = !(skill.locks || []).some(l => !isLockOpen(l));
    const reachable = !(parents[name] || []).length
      || parents[name].some(p => activatedSkills.has(p));
    return (skill.root || (unlocked && reachable)) ? "selectable" : "unselected";
  }

  function canLearn(name) {
    return learnable(name) && statusOf(name) === "selectable";
  }

  function setBg(g, name, hover) {
    const href = skillAsset(hover ? name + "_over" : name);
    if (g._href !== href) { g.bg.setAttribute("href", href); g._href = href; }
  }

  function svgImg(href, x, y, w, h, par) {
    return `<image href="${esc(href)}" x="${x}" y="${y}" width="${w}" height="${h}" preserveAspectRatio="${par || "xMidYMid meet"}"/>`;
  }

  function infoTitle(name) {
    const n = nodeByName[name];
    if (!n) return name;
    // 对应 skilltreebuilder.lua 的 gettitle：只有非信息板的锁才显示
    // 解锁/锁定文案，信息板（含天秤好/坏倾向）始终显示自己的标题。
    if (isLockNode(n) && !n.infographic) {
      return isLockOpen(name) ? "已解锁路径" : "路径锁定";
    }
    return n.title || n.name;
  }

  function updateInfoPanel() {
    const titleEle = $("#skTitle"), descEle = $("#skDesc");
    if (!focusing) {
      titleEle.textContent = "";
      descEle.textContent = "点击技能查看详情。";
      return;
    }
    const n = tree.nodes.find(m => m.name === focusing);
    titleEle.textContent = infoTitle(focusing);
    descEle.textContent = (n && n.desc) || "";
  }

  // 对应零件:Skilltree.js 的 switchLearnButton：左下按钮随焦点状态切换。
  function switchLearnButton() {
    const st = focusing ? statusOf(focusing) : null;
    if (st === "selected") {
      learnBtn.text.textContent = "已掌握技能";
      learnBtn.text.style.display = "";
      learnBtn.learned.style.display = "inline";
      learnBtn.normal.style.display = "none";
      learnBtn.hover.style.display = "none";
      learnBtn.down.style.display = "none";
    } else if (st === "selectable") {
      learnBtn.text.textContent = "学习";
      learnBtn.text.style.display = "";
      learnBtn.learned.style.display = "none";
      learnBtn.normal.style.display = "inline";
    } else {
      learnBtn.text.style.display = "none";
      learnBtn.learned.style.display = "none";
      learnBtn.normal.style.display = "none";
      learnBtn.hover.style.display = "none";
      learnBtn.down.style.display = "none";
    }
  }

  function learnFocused() {
    if (focusing && canLearn(focusing)) {
      activatedSkills.add(focusing);
      update();
    }
  }

  function resetSkills() {
    activatedSkills.clear();
    update();
  }

  // 装饰图亮度：未激活/未解锁的节点装饰调暗（对应游戏 button_decorations
  // 的 onlocked tint；解锁/掌握后恢复原色）。
  function updateDecoBrightness() {
    for (const name in decoGfx) {
      const n = nodeByName[name];
      const bright = (n && isLockNode(n) && isLockOpen(name)) || statusOf(name) === "selected";
      for (const el of decoGfx[name]) el.style.filter = bright ? "" : "brightness(0.5)";
    }
  }

  function update() {
    if (!tree) return;
    // 与游戏 RefreshTree 一致：统一按当前状态刷新所有已渲染节点底图。
    for (const name in gfx) {
      const g = gfx[name];
      setBg(g, baseTexture(name), g._hover);
    }
    updateDecoBrightness();
    $("#skillXp").textContent = `剩余洞察：${remainingXp()}`;
    const xpEle = $("#skXpNum");
    if (xpEle) xpEle.textContent = String(remainingXp());
    updateBalance();
    switchLearnButton();
    updateInfoPanel();
  }

  // 刷新沃拓克斯天秤读数（圆点 = 好/坏计数按阈值分档，与 UpdateToken 一致）。
  function updateBalance() {
    const layer = $("#skBalance");
    if (!layer) return;
    const inc = findInclination();
    const state = inc ? inclinationState(inc) : null;
    if (!state) { layer.style.display = "none"; return; }
    layer.style.display = "";
    const setDots = (role, count) => {
      layer.querySelectorAll(`circle[data-role="${role}"]`).forEach(c => {
        const index = Number(c.dataset.i) + 1;
        const on = count >= index;
        c.setAttribute("fill", on
          ? (role === "niceDot" ? "#e8c76a" : "#c96f5a")
          : "rgba(255,255,255,0.25)");
      });
    };
    // 与 UpdateToken 相同：diff 超出阈值时全部点亮（overcharged 只多发光）。
    setDots("niceDot", state.diff);
    setDots("naughtyDot", -state.diff);
    const text = layer.querySelector('[data-role="balanceText"]');
    const base = `好 ${state.nice} : ${state.naughty} 坏`;
    if (state.side === "nice") {
      text.textContent = `${base} · 好孩子倾向`;
      text.setAttribute("fill", "#e8c76a");
    } else if (state.side === "naughty") {
      text.textContent = `${base} · 淘气包倾向`;
      text.setAttribute("fill", "#c96f5a");
    } else {
      text.textContent = base;
      text.setAttribute("fill", "rgba(255,255,255,0.7)");
    }
  }

  function render() {
    const nodes = tree.nodes;
    if (!nodes.length) {
      $("#skwrap").innerHTML = '<p class="muted">该角色暂无技能树数据。</p>';
      return;
    }
    const inclination = findInclination();
    const meter = nodes.find(n => n.button_decorations && n.infographic);

    // 背景与连线（连接线由角色背景画承载，与游戏一致，不另画）。
    // 全屏弹窗背景 600x460 @ (0,-20)（playerinfopopupscreen.MakeBG）；
    // 角色背景按游戏 skilltreewidget.lua 的 521x320 @ (5,50) 绘制。
    const genericBg = skillAsset("background");
    const charBg = skillAsset(`${sel.value}_background`);

    // 图层顺序（SVG 按文档顺序绘制）：popup bg → decoration → char bg →
    // textbox → icon-bg → icon → focus → button。
    const parts = [];

    // 弹窗背景在游戏里由父屏幕绘制（playerinfopopupscreen.MakeBG），位于
    // 整个技能树部件之下，因此先画。
    parts.push(svgImg(genericBg, -WIDTH / 2, VIEW_TOP, WIDTH, SVG_HEIGHT, "none"));

    // 角色装饰图（薇诺娜货架等多背景）：画在角色背景之前，透过背景透明区显示。
    decoGfx = {};
    for (const n of nodes) {
      for (const d of (n.decorations || [])) {
        const cx = px(d.pos[0]), cy = py(d.pos[1]);
        if (d.size) {
          const [w, h] = d.size;
          parts.push(`<image class="decoration" data-name="${esc(n.name)}" href="${esc(skillAsset(d.img))}" x="${cx - w / 2}" y="${cy - h / 2}" width="${w}" height="${h}" preserveAspectRatio="none"/>`);
        } else {
          // 无显式尺寸：按原图 × scale，挂载后探测自然宽高再定位。
          parts.push(`<image class="decoration" data-name="${esc(n.name)}" data-scaled="1" data-scale="${d.scale || 1}" data-cx="${cx}" data-cy="${cy}" href="${esc(skillAsset(d.img))}" style="display:none"/>`);
        }
      }
    }
    parts.push(svgImg(charBg, BG_X, BG_Y, BG_ART_W, BG_ART_H, "none"));

    // 洞察面板（游戏 root.xp 在 (3,215)，沃拓克斯的天秤装饰会把它移到 x=0）。
    const x_xp = inclination ? 0 : XP_X;
    const y_xp = -XP_Y;
    parts.push(svgImg(skillAsset("skill_icon_textbox_white"), x_xp - XP_SIZE / 2, y_xp - XP_SIZE / 2, XP_SIZE, XP_SIZE));
    parts.push(`<text id="skXpNum" x="${x_xp}" y="${y_xp + 7}" text-anchor="middle" font-size="20" fill="white" class="unselectable">${remainingXp()}</text>`);
    parts.push(`<text x="${x_xp + 30}" y="${y_xp + 7}" font-size="15" fill="white" class="unselectable">剩余洞察</text>`);

    // 每个节点的状态底图 + 图标（信息板按钮/图标另有尺寸）。
    gfx = {};
    for (const n of nodes) {
      if (!n.icon && !isLockNode(n)) continue;
      const isLock = isLockNode(n);
      const x = px(n.x), y = py(n.y);
      const size = n.infographic
        ? INFO_BUTTON_SIZE
        : (isLock ? LOCK_SIZE : ICON_BUTTON_SIZE);
      const iconSize = n.infographic ? INFO_ICON_SIZE : ICON_SIZE;
      const bgName = baseTexture(n.name);
      parts.push(`<g class="node" data-name="${esc(n.name)}">`);
      parts.push(`<image data-role="bg" href="${esc(skillAsset(bgName))}" x="${x - size / 2}" y="${y - size / 2}" width="${size}" height="${size}" preserveAspectRatio="xMidYMid meet"/>`);
      if (n.icon) {
        parts.push(`<image data-role="icon" href="${esc(iconUrl(n.icon))}" x="${x - iconSize / 2}" y="${y - iconSize / 2}" width="${iconSize}" height="${iconSize}" preserveAspectRatio="xMidYMid meet"/>`);
      }
      parts.push(`</g>`);
      gfx[n.name] = { bg: null, icon: null, isLock, infographic: !!n.infographic, x, y, _hover: false, _href: skillAsset(bgName) };
    }

    // 沃拓克斯天秤读数：游戏用 wortox_balance 动画摆砝码，这里以等价的
    // 圆点 + 文本呈现好/坏计数与阈值（数量、修正规则与游戏一致）。
    if (inclination && meter) {
      const threshold = Number(inclination.threshold) || 0;
      const mx = px(meter.x), my = py(meter.y);
      const dotY = my + INFO_BUTTON_SIZE / 2 + 12;
      parts.push(`<g id="skBalance" pointer-events="none" style="display:none">`);
      for (let i = 0; i < threshold; i++) {
        parts.push(`<circle data-role="niceDot" data-i="${i}" cx="${mx - 16 - i * 11}" cy="${dotY}" r="4" fill="rgba(255,255,255,0.25)"/>`);
        parts.push(`<circle data-role="naughtyDot" data-i="${i}" cx="${mx + 16 + i * 11}" cy="${dotY}" r="4" fill="rgba(255,255,255,0.25)"/>`);
      }
      parts.push(`<text data-role="balanceText" class="unselectable" x="${mx}" y="${dotY + 20}" text-anchor="middle" font-size="13"></text>`);
      parts.push(`</g>`);
    }

    // 焦点框。
    parts.push(`<image data-role="skillFocus" href="${esc(skillAsset("frame"))}" width="${SKILL_FOCUS_SIZE}" height="${SKILL_FOCUS_SIZE}" style="display:none" pointer-events="none" preserveAspectRatio="xMidYMid meet"/>`);
    parts.push(`<image data-role="lockFocus" href="${esc(skillAsset("frame_octagon"))}" width="${LOCK_FOCUS_SIZE}" height="${LOCK_FOCUS_SIZE}" style="display:none" pointer-events="none" preserveAspectRatio="xMidYMid meet"/>`);

    // 底部按钮（左：学习；右：重置洞察）。
    const btnSrcs = { normal: reduxAsset("button_carny_long_normal"), hover: reduxAsset("button_carny_long_hover"), down: reduxAsset("button_carny_long_down") };
    parts.push(`<image data-role="learnLearned" pointer-events="none" href="${esc(skillAsset("skilltree_backgroundart"))}" x="${buttonLeftX}" y="${buttonY}" width="${buttonWidth}" height="${buttonHeight}" style="display:none" preserveAspectRatio="xMidYMid meet"/>`);
    for (const k of ["normal", "hover", "down"]) {
      parts.push(`<image data-role="learn${k}" pointer-events="none" href="${esc(btnSrcs[k])}" x="${buttonLeftX}" y="${buttonY}" width="${buttonWidth}" height="${buttonHeight}" preserveAspectRatio="xMidYMid meet"/>`);
    }
    parts.push(`<text data-role="learnText" pointer-events="none" class="st-button-text" x="${buttonLeftX + buttonWidth / 2}" y="${buttonY + buttonHeight / 2 + 6}" text-anchor="middle">学习</text>`);
    parts.push(`<rect data-role="learnProxy" x="${buttonLeftX}" y="${buttonY}" width="${buttonWidth}" height="${buttonHeight}" fill="transparent" style="cursor:pointer"/>`);
    for (const k of ["normal", "hover", "down"]) {
      parts.push(`<image data-role="reset${k}" pointer-events="none" href="${esc(btnSrcs[k])}" x="${buttonRightX}" y="${buttonY}" width="${buttonWidth}" height="${buttonHeight}" ${k === "normal" ? "" : 'style="display:none"'} preserveAspectRatio="xMidYMid meet"/>`);
    }
    parts.push(`<text data-role="resetText" pointer-events="none" class="st-button-text" x="${buttonRightX + buttonWidth / 2}" y="${buttonY + buttonHeight / 2 + 6}" text-anchor="middle">重置洞察</text>`);
    parts.push(`<rect data-role="resetProxy" x="${buttonRightX}" y="${buttonY}" width="${buttonWidth}" height="${buttonHeight}" fill="transparent" style="cursor:pointer"/>`);

    $("#skwrap").innerHTML =
      `<svg viewBox="${-WIDTH / 2} ${VIEW_TOP} ${WIDTH} ${SVG_HEIGHT}" width="100%" style="display:block;width:100%;height:auto;aspect-ratio:${WIDTH} / ${SVG_HEIGHT};background:#151923;border-radius:10px">
        ${parts.join("")}</svg>`;

    const svg = $("#skwrap svg");
    for (const name in gfx) {
      const g = gfx[name];
      const ele = svg.querySelector(`g.node[data-name="${CSS.escape(name)}"]`);
      g.bg = ele.querySelector('[data-role="bg"]');
      g.icon = ele.querySelector('[data-role="icon"]');
      ele.style.cursor = "pointer";
      ele.addEventListener("mouseenter", () => {
        g._hover = true;
        setBg(g, baseTexture(name), true);
        g.bg.parentElement.insertBefore(g.bg, g.icon || null);
      });
      ele.addEventListener("mouseleave", () => {
        g._hover = false;
        setBg(g, baseTexture(name), false);
      });
      ele.addEventListener("click", () => focusNode(name));
      // 游戏内双击技能按钮即学习。
      ele.addEventListener("dblclick", () => { if (canLearn(name)) { activatedSkills.add(name); update(); } });
    }

    // 装饰图：无显式尺寸的按原图 × scale 探测后居中；并按技能归组以便
    // 随解锁/学习状态调暗。
    for (const el of svg.querySelectorAll("image.decoration[data-scaled]")) {
      const probe = new Image();
      probe.onload = () => {
        const scale = Number(el.dataset.scale) || 1;
        const w = probe.naturalWidth * scale;
        const h = probe.naturalHeight * scale;
        el.setAttribute("x", Number(el.dataset.cx) - w / 2);
        el.setAttribute("y", Number(el.dataset.cy) - h / 2);
        el.setAttribute("width", w);
        el.setAttribute("height", h);
        el.style.display = "";
        updateDecoBrightness();
      };
      probe.src = el.getAttribute("href");
    }
    for (const el of svg.querySelectorAll("image.decoration")) {
      const name = el.dataset.name;
      (decoGfx[name] = decoGfx[name] || []).push(el);
    }

    skillFocusEle = svg.querySelector('[data-role="skillFocus"]');
    lockFocusEle = svg.querySelector('[data-role="lockFocus"]');
    learnBtn = {
      normal: svg.querySelector('[data-role="learnnormal"]'),
      hover: svg.querySelector('[data-role="learnhover"]'),
      down: svg.querySelector('[data-role="learndown"]'),
      learned: svg.querySelector('[data-role="learnLearned"]'),
      text: svg.querySelector('[data-role="learnText"]'),
    };
    resetBtn = {
      normal: svg.querySelector('[data-role="resetnormal"]'),
      hover: svg.querySelector('[data-role="resethover"]'),
      down: svg.querySelector('[data-role="resetdown"]'),
      text: svg.querySelector('[data-role="resetText"]'),
    };

    const learnProxy = svg.querySelector('[data-role="learnProxy"]');
    learnProxy.addEventListener("click", learnFocused);
    learnProxy.addEventListener("mouseenter", () => {
      if (focusing && statusOf(focusing) === "selectable") {
        learnBtn.normal.style.display = "none";
        learnBtn.hover.style.display = "inline";
      }
    });
    learnProxy.addEventListener("mouseleave", () => {
      if (focusing && statusOf(focusing) === "selectable") {
        learnBtn.hover.style.display = "none";
        learnBtn.normal.style.display = "inline";
      }
    });
    const resetProxy = svg.querySelector('[data-role="resetProxy"]');
    resetProxy.addEventListener("click", resetSkills);
    resetProxy.addEventListener("mouseenter", () => {
      resetBtn.normal.style.display = "none"; resetBtn.hover.style.display = "inline";
    });
    resetProxy.addEventListener("mouseleave", () => {
      resetBtn.hover.style.display = "none"; resetBtn.normal.style.display = "inline";
    });

    // 双击背景重置（对应 wiki 版本的 dblclick reset）。
    svg.querySelector(`image[href="${CSS.escape(skillAsset(`${sel.value}_background`))}"]`)?.addEventListener("dblclick", resetSkills);

    // 默认焦点（defaultfocus，控制器起点；缺失则取第一个根节点）。
    const defaultNode = nodes.find(n => n.defaultfocus) || nodes.find(n => n.root);
    if (defaultNode) focusNode(defaultNode.name, true);
    update();
  }

  function focusNode(name, silentScroll) {
    if (focusing !== name) {
      focusing = name;
      const g = gfx[name];
      if (g) {
        const n = nodeByName[name] || {};
        const isInfo = !!n.infographic;
        // 信息板即使带 lock_open 也用八角形以外的信息板边框（游戏里
        // infographic 的 frame 优先级高于 lock_open）。
        const useLockFrame = g.isLock && !isInfo;
        const focusEle = useLockFrame ? lockFocusEle : skillFocusEle;
        const other = useLockFrame ? skillFocusEle : lockFocusEle;
        const size = useLockFrame
          ? LOCK_FOCUS_SIZE
          : (isInfo ? INFO_FRAME_SIZE : SKILL_FOCUS_SIZE);
        if (!useLockFrame) {
          focusEle.setAttribute("href", skillAsset(isInfo ? "frame_infographic" : "frame"));
        }
        focusEle.setAttribute("x", g.x - size / 2);
        focusEle.setAttribute("y", g.y - size / 2);
        focusEle.setAttribute("width", size);
        focusEle.setAttribute("height", size);
        focusEle.style.display = "inline";
        other.style.display = "none";
      }
    }
    if (!silentScroll) update();
    else { switchLearnButton(); updateInfoPanel(); }
  }

  // 空格键学习当前焦点（对应 wiki 版本的 onKeydownSVG）。
  if (window.__skillKeyHandler) document.removeEventListener("keydown", window.__skillKeyHandler);
  window.__skillKeyHandler = (event) => {
    if (event.code !== "Space" || !tree) return;
    if (focusing && canLearn(focusing)) learnFocused();
    event.preventDefault();
  };
  document.addEventListener("keydown", window.__skillKeyHandler);

  async function draw() {
    tree = await getJSON(`/api/viz/skilltree?character=${sel.value}`);
    buildMaps(tree.nodes || []);
    activatedSkills.clear();
    focusing = null;
    render();
  }

  sel.onchange = () => { draw(); };

  await draw();
}
/* ---------------- inventory icons ---------------- */
