// 维基技能树渲染器（复制到灰机维基 零件:Skilltree.js；若站内有
// Gadget:Skilltree.js 副本也一并覆盖）
//
// 0.3.0 起坐标与游戏 widgets/redux/skilltreewidget.lua 和
// skilltreebuilder.lua 对齐：
//   - 角色背景按 521x320 绘制在 (5,50)
//   - 技能树 root 在 (0,-50)，按钮再偏移 -30 ⇒ 节点画面位置 (x, y-80)
//   - 洞察面板在 (3,165)，沃拓克斯（天秤）居中到 x=0
// 具体几何可由模块数据 `metainfo.render` 覆盖，渲染器不再硬编码旧偏移。
//
// 相对旧版的其它变化：
//   - 信息板（infographic）使用 infographic/infographic_on/infographic_off 贴图
//   - 支持 lock_open 里的 Inclination 条件（沃拓克斯好/坏倾向）
//   - 支持节点上的 decorations（薇诺娜货架等多背景图）
//   - 沃拓克斯天秤以圆点+文本等价呈现（游戏用 wortox_balance 动画砝码）
(() => {
    const VERSION = "0.3.0";
    console.log(`skilltree ${VERSION}`);
    const svgNamespace = "http://www.w3.org/2000/svg";
    const xlinkNamespace = "http://www.w3.org/1999/xlink";

    // 游戏常量（widgets/redux/skilltreebuilder.lua）
    const TILE = 32;
    const ICON_SIZE = TILE - 4;
    const INFO_ICON_SIZE = TILE - 5;
    const ICON_BUTTON_SIZE = TILE;
    const INFO_BUTTON_SIZE = (TILE - 5) * Math.SQRT2;
    const INFO_FRAME_SIZE = INFO_BUTTON_SIZE * (80 / 64) - 4;
    const LOCK_SIZE = TILE * 0.8;
    const FRAME_SIZE = TILE + 8;
    const TOTAL_XP = 15;

    // 渲染几何默认值；模块 `metainfo.render` 可逐项覆盖。
    const DEFAULT_RENDER = {
        width: 600,
        height: 460,
        view_top: -210,
        bg: { pos: [5, 50], size: [521, 320], tint: true },
        node_offset: [0, -80],
        xp: { pos: [3, 165] },
    };

    // 贴图名 -> metainfo.imgs 键（首字母大写）。
    const SKILL_TEXTURES = [
        "selected",
        "selected_over",
        "selectable",
        "selectable_over",
        "unselected",
        "unselected_over",
    ];
    const LOCK_TEXTURES = {
        locked: "locked_skill",
        locked_over: "locked_over",
        unlocked: "unlocked",
        unlocked_over: "unlocked_over",
    };
    const INFOGLOCK_TEXTURES = [
        "infographic_on",
        "infographic_on_over",
        "infographic_off",
        "infographic_off_over",
    ];
    const INFO_TEXTURES = ["infographic", "infographic_over"];

    // 读取每个 class 为 skilltree 的元素 为之构建技能树
    const skilltreeEles = document.getElementsByClassName("skilltree");
    for (const eleSkilltreeRoot of skilltreeEles) {
        buildSkilltree(eleSkilltreeRoot);
    }

    function capitalize(str) {
        return str.replace(/^./, (match) => match.toUpperCase());
    }

    // 主函数,构建技能树
    function buildSkilltree(rootEle) {
        const character = rootEle.dataset.character;
        const jsonEle = rootEle.querySelector(".def-json");
        const jsonObject = JSON.parse(jsonEle.innerText);
        const skilltreeDef = jsonObject.defs;
        const metainfo = jsonObject.metainfo || {};
        jsonEle.style.display = "none";

        const render = Object.assign({}, DEFAULT_RENDER, metainfo.render || {});
        if (!render.bg) render.bg = DEFAULT_RENDER.bg;
        if (!render.xp) render.xp = DEFAULT_RENDER.xp;
        render.node_offset = render.node_offset || DEFAULT_RENDER.node_offset;

        const WIDTH = render.width;
        const HEIGHT = render.height;
        // 视口：游戏部件根坐标（y 向上），SVG y 取负。
        const VIEW_X = -WIDTH / 2;
        const VIEW_TOP = render.view_top;
        const nodePx = (x) => x;
        const nodePy = (y) => -(y + render.node_offset[1]);
        const bgX = render.bg.pos[0] - render.bg.size[0] / 2;
        const bgY = -(render.bg.pos[1] + render.bg.size[1] / 2);

        const svgDummyEle = rootEle.querySelector(".to-svg");
        const svgElement = document.createElementNS(svgNamespace, "svg");
        svgElement.setAttribute("width", WIDTH);
        svgElement.setAttribute("height", HEIGHT);
        svgElement.setAttribute("viewBox", `${VIEW_X} ${VIEW_TOP} ${WIDTH} ${HEIGHT}`);
        svgElement.setAttribute("tabindex", "0");
        // 图层顺序：装饰 → 背景 → 文本 → 按钮底 → 图标 → 焦点框 → 底部按钮
        for (const groupName of [
            "decoration",
            "bg",
            "textbox",
            "icon-bg",
            "icon",
            "focus",
            "button",
        ]) {
            const groupEle = svgElement.appendChild(document.createElementNS(svgNamespace, "g"));
            groupEle.classList.add("skilltree-" + groupName);
        }
        svgDummyEle.parentNode.replaceChild(svgElement, svgDummyEle);
        const titleElement = rootEle.querySelector(".skilltree-skill-title");
        const descElement = rootEle.querySelector(".skilltree-skill-desc");
        const descriptionElement = rootEle.querySelector(".skilltree-skill-description-wrapper");

        const skills = {};
        const locks = {};
        const decorationsBySkill = {};
        let activatedSkills = [];
        let focusing = null;

        const btnsLeft = {};
        let eleTextBtnLeft;
        const btnsRight = {};
        let eleSkillFocus = null;
        let eleLockFocus = null;
        let balanceLayer = null;

        // ---- 工具函数 ----
        function skilltreeImg(name) {
            if (typeof mw !== "undefined") {
                const key = capitalize(name);
                const imgUrls = metainfo.imgs || {};
                if (imgUrls[key]) {
                    return imgUrls[key];
                }
                console.error(`cant find img for ${key}`);
                return null;
            }
            return `./images/skilltree/${name.toLowerCase()}.png`;
        }

        function globalReduxImg(name) {
            if (typeof mw !== "undefined") {
                const key = capitalize(name);
                const imgUrls = metainfo.imgs || {};
                if (imgUrls[key]) {
                    return imgUrls[key];
                }
                console.error(`cant find img for ${key}`);
                return null;
            }
            return `./images/global_redux/${name.toLowerCase()}.png`;
        }

        function skilltreeIconImg(skillName) {
            const def = skilltreeDef[skillName];
            if (typeof mw !== "undefined") {
                if (!def.icon_url) {
                    console.error(`cant find icon_url for ${skillName}`);
                    return null;
                }
                return def.icon_url;
            }
            return `./images/skilltree_icons/${def.icon}.png`;
        }

        function isLockDef(def) {
            return def.lock_open !== undefined && def.lock_open !== null;
        }

        function isInfoDef(def) {
            return !!def.infographic;
        }

        function isLearnable(def) {
            return !!def.icon && !isLockDef(def) && !isInfoDef(def);
        }

        function addImage(src, x, y, width, height, group, naturalScale) {
            if (!src) return null;
            const img = document.createElementNS(svgNamespace, "image");
            img.setAttributeNS(xlinkNamespace, "href", src);
            let parent = svgElement;
            if (group) parent = rootEle.querySelector("g." + group);
            if (width && height) {
                img.setAttribute("x", x);
                img.setAttribute("y", y);
                img.setAttribute("width", width);
                img.setAttribute("height", height);
                parent.appendChild(img);
            } else {
                // 未给尺寸时读取原图大小（装饰图的 SetScale 走这里）。
                const probe = new Image();
                probe.onload = () => {
                    const scale = naturalScale || 1;
                    const w = probe.naturalWidth * scale;
                    const h = probe.naturalHeight * scale;
                    img.setAttribute("x", x - w / 2);
                    img.setAttribute("y", y - h / 2);
                    img.setAttribute("width", w);
                    img.setAttribute("height", h);
                    parent.appendChild(img);
                };
                probe.src = src;
            }
            return img;
        }

        function createTextures(def, names, size, group, x, y) {
            def.btns = {};
            for (const name of names) {
                const imgName = LOCK_TEXTURES[name] || name;
                const src = skilltreeImg(imgName);
                def.btns[name] = addImage(
                    src,
                    x - size / 2,
                    y - size / 2,
                    size,
                    size,
                    group
                );
            }
        }

        function toggleTexture(def, status, hover) {
            const name = hover && def.btns[status + "_over"] ? status + "_over" : status;
            for (const key in def.btns) {
                const ele = def.btns[key];
                if (!ele) continue;
                ele.style.display = key === name ? "inline" : "none";
            }
        }

        function toggleBtns(btns, status) {
            for (const btnName in btns) {
                const btnEle = btns[btnName];
                if (!btnEle) continue;
                btnEle.style.display = btnName === status ? "inline" : "none";
            }
        }

        function changeText(ele, newText) {
            if (newText !== ele.textContent) ele.textContent = newText;
        }

        // ---- 倾向（沃拓克斯天秤） ----
        function countTags(tagName) {
            let count = 0;
            for (const skillName of activatedSkills) {
                const skill = skills[skillName];
                if (skill && skill.tags && skill.tags.includes(tagName)) count += 1;
            }
            return count;
        }

        // 复刻 skilltree_wortox.lua 的 CUSTOM_FUNCTIONS.CalculateInclination。
        function inclinationState(inc) {
            if (!inc || typeof inc !== "object") return null;
            const nice = Number(checkLockOpen(inc.nice)) || 0;
            const naughty = Number(checkLockOpen(inc.naughty)) || 0;
            let diff = nice - naughty;
            const affinity = checkLockOpen(inc.affinity);
            if (affinity) {
                if (diff < 0) diff -= 1;
                else if (diff > 0) diff += 1;
            }
            const threshold = Number(inc.threshold) || 0;
            const side =
                threshold > 0 && Math.abs(diff) >= threshold
                    ? diff > 0
                        ? "nice"
                        : "naughty"
                    : null;
            return { nice, naughty, diff, side, threshold, affinity: affinity || null };
        }

        function findInclination() {
            for (const name in skilltreeDef) {
                const cond = skilltreeDef[name].lock_open;
                const inc = cond && cond.Eq && cond.Eq.left && cond.Eq.left.Inclination;
                if (inc) return inc;
            }
            return null;
        }

        function checkLockOpen(lock_open) {
            if (typeof lock_open !== "object" || lock_open === null) {
                if (lock_open === undefined) {
                    console.error("unexpected undefined lock_open");
                    return false;
                }
                return lock_open;
            }
            for (const cond in lock_open) {
                switch (cond) {
                    case "GreaterThan":
                        return checkLockOpen(lock_open[cond].left) > checkLockOpen(lock_open[cond].right);
                    case "GreaterOrEqThan":
                        return checkLockOpen(lock_open[cond].left) >= checkLockOpen(lock_open[cond].right);
                    case "LessThan":
                        return checkLockOpen(lock_open[cond].left) < checkLockOpen(lock_open[cond].right);
                    case "LessOrEqThan":
                        return checkLockOpen(lock_open[cond].left) <= checkLockOpen(lock_open[cond].right);
                    case "Eq":
                        return checkLockOpen(lock_open[cond].left) === checkLockOpen(lock_open[cond].right);
                    case "And":
                        return checkLockOpen(lock_open[cond].left) && checkLockOpen(lock_open[cond].right);
                    case "Or":
                        return checkLockOpen(lock_open[cond].left) || checkLockOpen(lock_open[cond].right);
                    case "Not":
                        return !checkLockOpen(lock_open[cond]);
                    case "CountTags":
                        return countTags(lock_open.CountTags);
                    case "CountSkills":
                        return activatedSkills.length;
                    case "ActivatedSkill":
                        return activatedSkills.includes(lock_open.ActivatedSkill);
                    case "Add":
                        return checkLockOpen(lock_open[cond].left) + checkLockOpen(lock_open[cond].right);
                    case "Inclination": {
                        const state = inclinationState(lock_open[cond]);
                        return state ? state.side : null;
                    }
                    default:
                        console.error(`unexpected cond in lock_open ${cond}`);
                        return false;
                }
            }
            return false;
        }

        // ---- 交互 ----
        function toggleSkillDescription(skillName) {
            let has_desc = false;
            for (const descEle of descriptionElement.children) {
                if (descEle.dataset.skill === skillName) {
                    has_desc = true;
                    descEle.style.display = "block";
                } else {
                    descEle.style.display = "none";
                }
            }
            return has_desc;
        }

        function updateFocusing(skillName) {
            if (focusing !== skillName) {
                focusing = skillName;
                const def = skilltreeDef[skillName];
                if (isInfoDef(def)) {
                    titleElement.innerText = def.title || skillName;
                } else if (isLockDef(def)) {
                    titleElement.innerText = def.unlocked ? "已解锁路径" : "路径锁定";
                } else {
                    titleElement.innerText = def.title || skillName;
                }
                const has_description = toggleSkillDescription(skillName);
                descElement.innerText = has_description ? "" : def.desc || "";
            }
            switchLearnButton();
        }

        function switchLearnButton() {
            const def = focusing !== null ? skilltreeDef[focusing] : null;
            const status = def ? def.status : undefined;
            switch (status) {
                case "selected":
                    toggleBtns(btnsLeft, "learned");
                    changeText(eleTextBtnLeft, "已掌握技能");
                    break;
                case "selectable":
                    toggleBtns(btnsLeft, "normal");
                    changeText(eleTextBtnLeft, "学习");
                    break;
                default:
                    toggleBtns(btnsLeft, "disabled");
                    changeText(eleTextBtnLeft, "  ");
            }
        }

        function onKeydownSVG(event) {
            if (event.code !== "Space") return;
            if (focusing !== null) {
                const def = skilltreeDef[focusing];
                if (isLearnable(def) && def.status === "selectable") {
                    activatedSkills.push(focusing);
                    updateSkilltree();
                }
            }
            event.preventDefault();
        }

        // 焦点框：普通技能/信息板用方形（信息板用更大边框），锁用八角。
        function showFocus(def, x_pos, y_pos) {
            const info = isInfoDef(def);
            const lock = isLockDef(def) && !info;
            const size = lock ? FRAME_SIZE : info ? INFO_FRAME_SIZE : FRAME_SIZE;
            const ele = lock ? eleLockFocus : eleSkillFocus;
            const other = lock ? eleSkillFocus : eleLockFocus;
            if (!lock) {
                ele.setAttributeNS(
                    xlinkNamespace,
                    "href",
                    skilltreeImg(info ? "frame_infographic" : "frame") || ""
                );
            }
            ele.setAttribute("x", x_pos - size / 2);
            ele.setAttribute("y", y_pos - size / 2);
            ele.setAttribute("width", size);
            ele.setAttribute("height", size);
            ele.style.display = "inline";
            other.style.display = "none";
        }

        function statusTexture(def) {
            if (isLockDef(def)) {
                const open = !!checkLockOpen(def.lock_open);
                if (isInfoDef(def)) return open ? "infographic_on" : "infographic_off";
                return open ? "unlocked" : "locked";
            }
            if (isInfoDef(def)) return "infographic";
            return def.status || "unselected";
        }

        // ---- 更新 ----
        function updateDecorations() {
            for (const skillName in decorationsBySkill) {
                const def = skilltreeDef[skillName];
                const bright =
                    def.status === "selected" || (locks[skillName] && locks[skillName].unlocked);
                for (const ele of decorationsBySkill[skillName]) {
                    ele.style.filter = bright ? "" : "brightness(0.5)";
                }
            }
        }

        function updateBalance(inclination) {
            if (!balanceLayer || !inclination) return;
            const state = inclinationState(inclination);
            if (!state) {
                balanceLayer.style.display = "none";
                return;
            }
            balanceLayer.style.display = "";
            const setDots = (role, count) => {
                balanceLayer.querySelectorAll(`circle[data-role="${role}"]`).forEach((c) => {
                    const index = Number(c.dataset.i) + 1;
                    const on = count >= index;
                    c.setAttribute(
                        "fill",
                        on
                            ? role === "niceDot"
                                ? "#e8c76a"
                                : "#c96f5a"
                            : "rgba(255,255,255,0.25)"
                    );
                });
            };
            setDots("niceDot", state.diff);
            setDots("naughtyDot", -state.diff);
            const text = balanceLayer.querySelector('[data-role="balanceText"]');
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

        function updateSkilltree() {
            // 先锁：刷新贴图与解锁状态。
            for (const lockName in locks) {
                const lockDef = locks[lockName];
                const unlocked = !!checkLockOpen(lockDef.lock_open);
                if (unlocked !== lockDef.unlocked) {
                    lockDef.unlocked = unlocked;
                }
                toggleTexture(lockDef, statusTexture(lockDef), false);
            }
            // 再技能：激活/可选/不可选。
            for (const skillName in skills) {
                const def = skills[skillName];
                const has_points = TOTAL_XP - activatedSkills.length > 0;
                if (activatedSkills.includes(skillName)) {
                    def.status = "selected";
                } else {
                    const has_lock = def.locks !== undefined;
                    let unlocked = !has_lock;
                    if (has_lock) {
                        unlocked = def.locks
                            .map((x) => (locks[x] ? checkLockOpen(locks[x].lock_open) : true))
                            .reduce((a, b) => a && b, true);
                    }
                    const reachable = !def.parent || def.parent.some((x) => activatedSkills.includes(x));
                    def.status =
                        has_points && (def.root || (unlocked && reachable))
                            ? "selectable"
                            : "unselected";
                }
                toggleTexture(def, statusTexture(def), false);
            }
            updateDecorations();

            const inclination = findInclination();
            updateBalance(inclination);

            // 更新 XP 文本。
            const eleSkillXPText = rootEle.querySelector(".skilltree-xp");
            const xp = TOTAL_XP - activatedSkills.length;
            eleSkillXPText.textContent = xp.toString();

            switchLearnButton();
        }

        function init() {
            // 装饰图（多背景，如薇诺娜的货架）先画，角色背景透明处透出。
            for (const skillName in skilltreeDef) {
                const def = skilltreeDef[skillName];
                if (!def.decorations || def.decorations.length === 0) continue;
                decorationsBySkill[skillName] = [];
                for (const dec of def.decorations) {
                    const src = skilltreeImg(dec.img);
                    const cx = dec.pos[0];
                    const cy = dec.pos[1];
                    let ele;
                    if (dec.size) {
                        ele = addImage(
                            src,
                            cx - dec.size[0] / 2,
                            -cy - dec.size[1] / 2,
                            dec.size[0],
                            dec.size[1],
                            "skilltree-decoration"
                        );
                    } else {
                        // 原图尺寸 × scale，加载完成后按中心对齐。
                        ele = addImage(src, cx, -cy, null, null, "skilltree-decoration", dec.scale || 1);
                    }
                    if (ele) {
                        ele.dataset.skillName = skillName;
                        decorationsBySkill[skillName].push(ele);
                    }
                }
            }

            // 整个背景（含连线）。
            if (render.bg) {
                const srcCharacterBG = skilltreeImg(character + "_background");
                const eleBG = addImage(
                    srcCharacterBG,
                    bgX,
                    bgY,
                    render.bg.size[0],
                    render.bg.size[1],
                    "skilltree-bg"
                );
                if (eleBG) {
                    if (render.bg.tint !== false) eleBG.classList.add("tint-gold");
                    eleBG.addEventListener("dblclick", () => {
                        activatedSkills = [];
                        updateSkilltree();
                    });
                }
            }

            // XP 部分（沃拓克斯天秤会把它居中）。
            const inclination = findInclination();
            const xpX = inclination ? 0 : render.xp.pos[0];
            const xpY = -render.xp.pos[1];
            const XP_SIZE = 50;
            const eleSkillIconTextBox = addImage(
                skilltreeImg("skill_icon_textbox_white"),
                xpX - XP_SIZE / 2,
                xpY - XP_SIZE / 2,
                XP_SIZE,
                XP_SIZE,
                "skilltree-textbox"
            );
            if (eleSkillIconTextBox) eleSkillIconTextBox.classList.add("tint-gold");

            const eleTextboxGroup = rootEle.querySelector(".skilltree-textbox");
            const eleSkillXPText = document.createElementNS(svgNamespace, "text");
            eleSkillXPText.setAttribute("font-size", 20);
            eleSkillXPText.setAttribute("x", xpX);
            eleSkillXPText.setAttribute("y", xpY + 8);
            eleSkillXPText.setAttribute("text-anchor", "middle");
            eleSkillXPText.setAttribute("font-family", "HammerheadRegular");
            eleSkillXPText.setAttribute("fill", "white");
            eleSkillXPText.classList.add("skilltree-xp");
            eleSkillXPText.classList.add("tint-gold");
            eleSkillXPText.classList.add("unselectable");
            eleSkillXPText.textContent = TOTAL_XP.toString();
            eleTextboxGroup.appendChild(eleSkillXPText);

            const eleSkillHintText = document.createElementNS(svgNamespace, "text");
            eleSkillHintText.setAttribute("x", xpX + 30);
            eleSkillHintText.setAttribute("y", xpY + 8);
            eleSkillHintText.setAttribute("font-family", "HammerheadRegular");
            eleSkillHintText.setAttribute("font-size", 15);
            eleSkillHintText.setAttribute("fill", "white");
            eleSkillHintText.classList.add("tint-gold");
            eleSkillHintText.classList.add("unselectable");
            eleSkillHintText.textContent = "剩余洞察";
            eleTextboxGroup.appendChild(eleSkillHintText);

            // 焦点框（普通/信息板/锁）。
            eleSkillFocus = addImage(
                skilltreeImg("frame"),
                0,
                0,
                FRAME_SIZE,
                FRAME_SIZE,
                "skilltree-focus"
            );
            eleLockFocus = addImage(
                skilltreeImg("frame_octagon"),
                0,
                0,
                FRAME_SIZE,
                FRAME_SIZE,
                "skilltree-focus"
            );
            if (eleSkillFocus) eleSkillFocus.style.display = "none";
            if (eleLockFocus) eleLockFocus.style.display = "none";

            // 学习和重置按钮（几何与旧版一致，换算到居中视口）。
            const buttonWidth = 180;
            const buttonHeight = 43;
            const buttonLeftX = -130 - buttonWidth / 2;
            const buttonRightX = 130 - buttonWidth / 2;
            const buttonY = 150;
            const srcsButton = {
                normal: globalReduxImg("button_carny_long_normal"),
                down: globalReduxImg("button_carny_long_down"),
                hover: globalReduxImg("button_carny_long_hover"),
            };
            for (const src of ["normal", "down", "hover"]) {
                btnsLeft[src] = addImage(
                    srcsButton[src],
                    buttonLeftX,
                    buttonY,
                    buttonWidth,
                    buttonHeight,
                    "skilltree-button"
                );
                btnsRight[src] = addImage(
                    srcsButton[src],
                    buttonRightX,
                    buttonY,
                    buttonWidth,
                    buttonHeight,
                    "skilltree-button"
                );
            }
            btnsLeft.learned = addImage(
                skilltreeImg("skilltree_backgroundart"),
                buttonLeftX,
                buttonY,
                buttonWidth,
                buttonHeight,
                "skilltree-button"
            );
            eleTextBtnLeft = document.createElementNS(svgNamespace, "text");
            eleTextBtnLeft.textContent = "学习";
            eleTextBtnLeft.setAttribute("text-anchor", "middle");
            eleTextBtnLeft.classList.add("button-text");
            svgElement.appendChild(eleTextBtnLeft);
            eleTextBtnLeft.setAttribute("x", buttonLeftX + buttonWidth / 2);
            eleTextBtnLeft.setAttribute("y", buttonY + buttonHeight / 2 + 6);

            const btnProxyLeft = document.createElementNS(svgNamespace, "rect");
            svgElement.appendChild(btnProxyLeft);
            btnProxyLeft.dataset.role = "learn";
            btnProxyLeft.setAttribute("x", buttonLeftX);
            btnProxyLeft.setAttribute("y", buttonY);
            btnProxyLeft.setAttribute("width", buttonWidth);
            btnProxyLeft.setAttribute("height", buttonHeight);
            btnProxyLeft.setAttribute("fill", "transparent");
            btnProxyLeft.addEventListener("click", () => {
                if (focusing !== null && skilltreeDef[focusing].status === "selectable") {
                    activatedSkills.push(focusing);
                    updateSkilltree();
                }
            });
            btnProxyLeft.addEventListener("mouseenter", () => {
                if (focusing !== null && skilltreeDef[focusing].status === "selectable") {
                    toggleBtns(btnsLeft, "hover");
                }
            });
            btnProxyLeft.addEventListener("mouseleave", () => {
                if (focusing !== null && skilltreeDef[focusing].status === "selectable") {
                    toggleBtns(btnsLeft, "normal");
                }
            });
            btnProxyLeft.addEventListener("mousedown", () => {
                if (focusing !== null && skilltreeDef[focusing].status === "selectable") {
                    toggleBtns(btnsLeft, "down");
                }
            });
            btnProxyLeft.addEventListener("mouseup", () => {
                if (focusing !== null && skilltreeDef[focusing].status === "selectable") {
                    toggleBtns(btnsLeft, "normal");
                }
            });

            const btnTextRight = document.createElementNS(svgNamespace, "text");
            btnTextRight.textContent = "重置洞察";
            btnTextRight.setAttribute("text-anchor", "middle");
            btnTextRight.classList.add("button-text");
            svgElement.appendChild(btnTextRight);
            btnTextRight.setAttribute("x", buttonRightX + buttonWidth / 2);
            btnTextRight.setAttribute("y", buttonY + buttonHeight / 2 + 6);

            const btnProxyRight = document.createElementNS(svgNamespace, "rect");
            svgElement.appendChild(btnProxyRight);
            btnProxyRight.dataset.role = "reset";
            btnProxyRight.setAttribute("x", buttonRightX);
            btnProxyRight.setAttribute("y", buttonY);
            btnProxyRight.setAttribute("width", buttonWidth);
            btnProxyRight.setAttribute("height", buttonHeight);
            btnProxyRight.setAttribute("fill", "transparent");
            btnProxyRight.addEventListener("click", () => {
                activatedSkills = [];
                updateSkilltree();
            });
            btnProxyRight.addEventListener("mouseenter", () => toggleBtns(btnsRight, "hover"));
            btnProxyRight.addEventListener("mouseleave", () => toggleBtns(btnsRight, "normal"));
            btnProxyRight.addEventListener("mousedown", () => toggleBtns(btnsRight, "down"));
            btnProxyRight.addEventListener("mouseup", () => toggleBtns(btnsRight, "normal"));
            toggleBtns(btnsRight, "normal");

            // 沃拓克斯天秤读数（游戏用 wortox_balance 动画砝码）。
            if (inclination) {
                let meter = null;
                for (const name in skilltreeDef) {
                    const def = skilltreeDef[name];
                    if (def.button_decorations && def.infographic) {
                        meter = def;
                        break;
                    }
                }
                if (meter) {
                    const mx = nodePx(meter.pos[0]);
                    const my = nodePy(meter.pos[1]);
                    const dotY = my + INFO_BUTTON_SIZE / 2 + 12;
                    const threshold = Number(inclination.threshold) || 0;
                    balanceLayer = document.createElementNS(svgNamespace, "g");
                    balanceLayer.setAttribute("pointer-events", "none");
                    balanceLayer.style.display = "none";
                    for (let i = 0; i < threshold; i++) {
                        for (const role of ["niceDot", "naughtyDot"]) {
                            const dot = document.createElementNS(svgNamespace, "circle");
                            dot.dataset.role = role;
                            dot.dataset.i = String(i);
                            dot.setAttribute("cy", dotY);
                            dot.setAttribute(
                                "cx",
                                role === "niceDot" ? mx - 16 - i * 11 : mx + 16 + i * 11
                            );
                            dot.setAttribute("r", 4);
                            dot.setAttribute("fill", "rgba(255,255,255,0.25)");
                            balanceLayer.appendChild(dot);
                        }
                    }
                    const label = document.createElementNS(svgNamespace, "text");
                    label.dataset.role = "balanceText";
                    label.classList.add("unselectable");
                    label.setAttribute("x", mx);
                    label.setAttribute("y", dotY + 20);
                    label.setAttribute("text-anchor", "middle");
                    label.setAttribute("font-size", 13);
                    balanceLayer.appendChild(label);
                    svgElement.appendChild(balanceLayer);
                }
            }

            // 遍历树定义 添加技能/锁/信息板。
            for (const skillName in skilltreeDef) {
                const def = skilltreeDef[skillName];
                const x_pos = nodePx(def.pos[0]);
                const y_pos = nodePy(def.pos[1]);
                const info = isInfoDef(def);
                const lock = isLockDef(def);

                if (def.icon) {
                    // 父技能（connects 指向它的技能），锁的连接由子节点 locks 获得。
                    if (def.connects) {
                        for (const childSkillName of def.connects) {
                            const childSkill = skilltreeDef[childSkillName];
                            if (!childSkill.parent) childSkill.parent = [];
                            childSkill.parent.push(skillName);
                        }
                    }
                    const iconSize = info ? INFO_ICON_SIZE : ICON_SIZE;
                    const ele = (def.iconEle = addImage(
                        skilltreeIconImg(skillName),
                        x_pos - iconSize / 2,
                        y_pos - iconSize / 2,
                        iconSize,
                        iconSize,
                        "skilltree-icon"
                    ));
                    if (ele) {
                        ele.dataset.skillName = skillName;
                        ele.addEventListener("mouseenter", function () {
                            const d = skilltreeDef[this.dataset.skillName];
                            if (isLearnable(d)) toggleTexture(d, d.status, true);
                            else toggleTexture(d, statusTexture(d), true);
                        });
                        ele.addEventListener("mouseleave", function () {
                            const d = skilltreeDef[this.dataset.skillName];
                            if (isLearnable(d)) toggleTexture(d, d.status, false);
                            else toggleTexture(d, statusTexture(d), false);
                        });
                        ele.addEventListener("click", function () {
                            const name = this.dataset.skillName;
                            if (focusing !== name) {
                                updateFocusing(name);
                                showFocus(skilltreeDef[name], x_pos, y_pos);
                            }
                            svgElement.focus({ preventScroll: true });
                        });
                    }
                }

                // 底图贴图：信息板锁用 on/off，信息板用 infographic，
                // 普通锁用 locked/unlocked，普通技能用三态。
                let textures;
                let buttonSize;
                if (info && lock) {
                    textures = INFOGLOCK_TEXTURES;
                    buttonSize = INFO_BUTTON_SIZE;
                } else if (info) {
                    textures = INFO_TEXTURES;
                    buttonSize = INFO_BUTTON_SIZE;
                } else if (lock) {
                    textures = Object.keys(LOCK_TEXTURES);
                    buttonSize = LOCK_SIZE;
                } else {
                    textures = SKILL_TEXTURES;
                    buttonSize = ICON_BUTTON_SIZE;
                }
                createTextures(def, textures, buttonSize, "skilltree-icon-bg", x_pos, y_pos);
                if (lock) {
                    def.unlocked = !!checkLockOpen(def.lock_open);
                }
                toggleTexture(def, statusTexture(def), false);

                // 所有节点都可点击聚焦；锁/信息板追加各自的点击与悬停。
                for (const key in def.btns) {
                    const btnEle = def.btns[key];
                    if (!btnEle) continue;
                    btnEle.dataset.skillName = skillName;
                    btnEle.addEventListener("click", function () {
                        const name = this.dataset.skillName;
                        if (focusing !== name) {
                            updateFocusing(name);
                            showFocus(skilltreeDef[name], x_pos, y_pos);
                        }
                        svgElement.focus({ preventScroll: true });
                    });
                    btnEle.addEventListener("mouseenter", function () {
                        const d = skilltreeDef[this.dataset.skillName];
                        if (isLearnable(d)) return;
                        toggleTexture(d, statusTexture(d), true);
                    });
                    btnEle.addEventListener("mouseleave", function () {
                        const d = skilltreeDef[this.dataset.skillName];
                        if (isLearnable(d)) return;
                        toggleTexture(d, statusTexture(d), false);
                    });
                }

                if (isLockDef(def)) {
                    // 只有一个 lock 的 skill 会没有 locks,需要在这里补上。
                    if (def.connects) {
                        for (const childSkillName of def.connects) {
                            const childSkillDef = skilltreeDef[childSkillName];
                            if (!childSkillDef.locks) childSkillDef.locks = [];
                            if (!childSkillDef.locks.includes(skillName)) {
                                childSkillDef.locks.push(skillName);
                            }
                        }
                    }
                    locks[skillName] = def;
                }
                if (isLearnable(def)) {
                    skills[skillName] = def;
                }

                if (def.defaultfocus) {
                    updateFocusing(skillName);
                    showFocus(def, x_pos, y_pos);
                }
            }
            updateSkilltree();

            // 空格键学习（需先让 SVG 获得焦点）。
            svgElement.addEventListener("keydown", onKeydownSVG);
        }

        init();
    }
})();
