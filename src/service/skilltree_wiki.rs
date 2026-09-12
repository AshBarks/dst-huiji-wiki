//! 模块:Skilltree/<Character> 子页面维护。
//!
//! 用技能树解析器把游戏 `skilltree_<char>.lua` 提取成子页面的 `defs` JSON
//! （与灰机维基 `模块:Skilltree` 前端零件:Skilltree.js 消费的格式一致）。
//! 更新已有子页面时保留页内 `metainfo`（以及旧 defs 里的 `icon_url`，
//! 那是维基站内上传图片后才有的地址，本地无法生成）。
//!
//! `metainfo` 里除 `imgs` 外还写入 `render`（游戏部件几何：背景矩形、
//! 节点偏移、XP 位置、背景 tint），渲染器据此布局而无需硬编码常量；
//! `imgs` 会与角色所需图片清单合并（缺失键留空并计入报告），随后用
//! `prop=imageinfo&iiprop=url|size` 批量查回已上传图片的真实 URL（含
//! `defs[].icon_url`），只有站内确实没有的才留在报告里待手工上传；
//! 角色背景图还会校验是否为游戏原图尺寸（625×384），站内历史上的
//! 拉伸放大版（852×756 / 852×653）会计入 `img_size_warnings`。
//! `--output` 目录同时写出 `Skilltree.js`（更新版渲染器，复制到维基
//! `零件:Skilltree.js`）。

use super::dataset::{load_skill_strings, read_game_file};
use crate::error::Result;
use crate::parser::skilltree::{parse_skill_tree_with_tuning, SkillNode, SkillTree};
use crate::platform::progress::Reporter;
use crate::platform::progress::{decide_write, WriteDecision, WriteMode};
use crate::wiki::WikiClient;
use std::path::PathBuf;

/// 维基渲染器源码（`零件:Skilltree.js`），随 `--output` 写出供手工上传。
pub const SKILLTREE_WIDGET_JS: &str = include_str!("assets/skilltree_widget.js");

/// 一级命名空间内的子页面标题，例如 `模块:Skilltree/Walter`。
fn page_title(character: &str) -> String {
    let mut chars = character.chars();
    let head = chars.next().map(|c| c.to_uppercase().to_string());
    match head {
        Some(h) => format!("模块:Skilltree/{}{}", h, chars.as_str()),
        None => "模块:Skilltree".to_string(),
    }
}

/// 与 `零件:Skilltree.js` 的 `capitalize` 保持一致（仅首字母大写）。
fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(head) => head.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// 所有技能树都需要的通用图片（键名与维基已上传文件一致）。
const COMMON_IMGS: &[&str] = &[
    "Frame",
    "Frame_octagon",
    "Selectable",
    "Selectable_over",
    "Selected",
    "Selected_over",
    "Unselected",
    "Unselected_over",
    "Locked_skill",
    "Locked_over",
    "Unlocked",
    "Unlocked_over",
    "Skill_icon_textbox_white",
    "Skilltree_backgroundart",
    "Button_carny_long_normal",
    "Button_carny_long_hover",
    "Button_carny_long_down",
];

/// 历史遗留的“逻辑键名 ≠ 站内文件名”映射（键 → 文件主干）。
///
/// 现有子页面里 `Button_carny_long_*` 三个键指向 `Button_long_*.png`，
/// 新页面自动回填 URL 时需要按实际文件名查询。
const IMG_FILE_ALIASES: &[(&str, &str)] = &[
    ("Button_carny_long_normal", "Button_long_normal"),
    ("Button_carny_long_hover", "Button_long_hover"),
    ("Button_carny_long_down", "Button_long_down"),
];

/// 游戏角色背景原图的尺寸（`widgets/redux/skilltreewidget.lua` 断言值）。
///
/// 站内早期上传过 852×756 / 852×653 的拉伸放大版，旧渲染器用 Y_SCALE
/// 硬编码适配；新渲染器按游戏几何 521×320 绘制，这些图会明显偏小。
const GAME_BG_SIZE: (u32, u32) = (625, 384);

/// 逻辑图片键对应的站内文件名主干（不含扩展名）。
fn img_file_stem(key: &str) -> &str {
    IMG_FILE_ALIASES
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, stem)| *stem)
        .unwrap_or(key)
}

/// 含信息板（infographic）的树才需要的图片。
const INFOGRAPHIC_IMGS: &[&str] = &[
    "Frame_infographic",
    "Infographic",
    "Infographic_over",
    "Infographic_on",
    "Infographic_on_over",
    "Infographic_off",
    "Infographic_off_over",
];

/// 该角色模块需要登记的 `metainfo.imgs` 键（含角色背景与装饰图）。
fn expected_img_keys(tree: &SkillTree) -> Vec<String> {
    let mut keys: Vec<String> = COMMON_IMGS.iter().map(|s| s.to_string()).collect();
    keys.push(format!("{}_background", capitalize(&tree.character)));
    if tree.nodes.iter().any(|n| n.infographic) {
        keys.extend(INFOGRAPHIC_IMGS.iter().map(|s| s.to_string()));
    }
    let mut decorations: Vec<String> = tree
        .nodes
        .iter()
        .flat_map(|n| n.decorations.iter().map(|d| capitalize(&d.img)))
        .collect();
    decorations.sort();
    decorations.dedup();
    keys.extend(decorations);
    keys
}

/// 渲染器读取的游戏部件几何（与 `widgets/redux/skilltreewidget.lua` 和
/// `skilltreebuilder.lua` 对应）。
fn render_meta(tree: &SkillTree) -> serde_json::Value {
    let tint = tree
        .background
        .as_ref()
        .and_then(|b| b.tint_bright)
        .unwrap_or(true);
    serde_json::json!({
        "bg": {
            "pos": [5, 50],
            "size": [521, 320],
            "tint": tint,
        },
        "node_offset": [0, -80],
        "xp": { "pos": [3, 165] },
    })
}

/// 把一个 SkillNode 转成子页面 defs 里的一个条目。
///
/// 键序对齐维基现有手写 JSON：group/root/connects/locks/lock_open/tags/
/// pos/title/desc/icon/icon_url + 存在性标记。
fn def_to_json(
    node: &SkillNode,
    title: Option<&String>,
    desc: Option<&String>,
) -> serde_json::Value {
    let mut obj = serde_json::Map::new();
    if let Some(group) = &node.group {
        obj.insert("group".into(), serde_json::json!(group));
    }
    if node.root {
        obj.insert("root".into(), serde_json::json!(true));
    }
    if !node.connects.is_empty() {
        obj.insert("connects".into(), serde_json::json!(node.connects));
    }
    if !node.locks.is_empty() {
        obj.insert("locks".into(), serde_json::json!(node.locks));
    }
    if let Some(lock_open) = &node.lock_open {
        obj.insert("lock_open".into(), lock_open.clone());
    }
    if !node.tags.is_empty() {
        obj.insert("tags".into(), serde_json::json!(node.tags));
    }
    obj.insert("pos".into(), serde_json::json!([node.x, node.y]));
    if let Some(t) = title {
        obj.insert("title".into(), serde_json::json!(t));
    }
    if let Some(d) = desc {
        obj.insert("desc".into(), serde_json::json!(d));
    }
    if let Some(icon) = &node.icon {
        obj.insert("icon".into(), serde_json::json!(icon));
    }
    if node.onactivate {
        obj.insert("onactivate".into(), serde_json::json!(true));
    }
    if node.ondeactivate {
        obj.insert("ondeactivate".into(), serde_json::json!(true));
    }
    if node.defaultfocus {
        obj.insert("defaultfocus".into(), serde_json::json!(true));
    }
    if node.infographic {
        obj.insert("infographic".into(), serde_json::json!(true));
    }
    if let Some(ff) = &node.forced_focus {
        obj.insert("forced_focus".into(), ff.clone());
    }
    if node.button_decorations {
        obj.insert("button_decorations".into(), serde_json::json!(true));
    }
    if !node.decorations.is_empty() {
        obj.insert(
            "decorations".into(),
            serde_json::to_value(&node.decorations).unwrap_or(serde_json::Value::Null),
        );
    }
    serde_json::Value::Object(obj)
}

/// 从维基子页面文本 `return [[ ... ]]` 中取出 JSON 对象。
fn parse_page_json(content: &str) -> Option<serde_json::Value> {
    let trimmed = content.trim();
    let inner = trimmed
        .strip_prefix("return [[")
        .and_then(|s| s.strip_suffix("]]"))?;
    serde_json::from_str(inner).ok()
}

/// 页面组装结果：内容 + 缺失资源报告（供 CLI 输出与 `--report-json`）。
struct BuiltPage {
    content: serde_json::Value,
    missing_imgs: Vec<String>,
    missing_icon_urls: Vec<String>,
}

/// 解析“已有页面内容”为 defs JSON。
///
/// - 页面缺失或内容为空 → `Ok(None)`（走新建分支）；
/// - 内容可解析 → `Ok(Some(v))`；
/// - 内容非空但解析失败 → `Err(())`（调用方跳过该角色，不得覆盖写）。
fn parse_existing(content: Option<&str>) -> std::result::Result<Option<serde_json::Value>, ()> {
    match content {
        Some(c) if !c.trim().is_empty() => parse_page_json(c).map(Some).ok_or(()),
        _ => Ok(None),
    }
}

/// 用新提取的 defs 组装子页面内容；已有页面保留 metainfo 与 icon_url。
///
/// `metainfo.imgs` 按所需键合并：已有 URL 保留，缺失键写入空串并在报告里
/// 列出（上传后下次运行会自动保留站内填好的 URL）。其余旧 `metainfo` 键
/// （如未来新增的配置）原样保留。
fn build_page_content(
    tree: &SkillTree,
    strings: &std::collections::BTreeMap<String, String>,
    existing: Option<&serde_json::Value>,
) -> BuiltPage {
    let character = tree.character.as_str();
    let upper_char = character.to_uppercase();
    let prefix = format!("STRINGS.SKILLTREE.{}.", upper_char);

    let old_defs = existing
        .and_then(|v| v.get("defs"))
        .and_then(|d| d.as_object())
        .cloned()
        .unwrap_or_default();

    let mut missing_icon_urls = Vec::new();
    let mut defs = serde_json::Map::new();
    for node in &tree.nodes {
        let title_key = format!("{}{}_TITLE", prefix, node.name.to_uppercase());
        let desc_key = format!("{}{}_DESC", prefix, node.name.to_uppercase());
        let mut def = def_to_json(node, strings.get(&title_key), strings.get(&desc_key));
        // 维基侧维护的 icon_url（站内图片地址）原样保留。
        match old_defs.get(&node.name).and_then(|d| d.get("icon_url")) {
            Some(icon_url) => {
                if let Some(obj) = def.as_object_mut() {
                    obj.insert("icon_url".into(), icon_url.clone());
                }
            }
            None if node.icon.is_some() => missing_icon_urls.push(node.name.clone()),
            None => {}
        }
        defs.insert(node.name.clone(), def);
    }

    // --- metainfo：合并 imgs 清单 + 渲染几何 ---
    let old_metainfo = existing
        .and_then(|v| v.get("metainfo"))
        .and_then(|m| m.as_object())
        .cloned()
        .unwrap_or_default();
    let old_imgs = old_metainfo
        .get("imgs")
        .and_then(|m| m.as_object())
        .cloned()
        .unwrap_or_default();

    let mut imgs = serde_json::Map::new();
    let mut missing_imgs = Vec::new();
    for key in expected_img_keys(tree) {
        match old_imgs.get(&key) {
            Some(url) if !url.as_str().unwrap_or("").is_empty() => {
                imgs.insert(key, url.clone());
            }
            _ => {
                missing_imgs.push(key.clone());
                imgs.insert(key, serde_json::json!(""));
            }
        }
    }
    // 保留页面上额外登记、但不在清单里的图片键。
    for (k, v) in &old_imgs {
        if !imgs.contains_key(k) {
            imgs.insert(k.clone(), v.clone());
        }
    }

    let mut metainfo = old_metainfo;
    metainfo.insert("imgs".into(), serde_json::Value::Object(imgs));
    metainfo.insert("render".into(), render_meta(tree));

    let mut page = serde_json::Map::new();
    page.insert("defs".into(), serde_json::Value::Object(defs));
    page.insert("metainfo".into(), serde_json::Value::Object(metainfo));
    // 保留旧页面的其余顶层键，尊重站内后续扩展。
    if let Some(existing_obj) = existing.and_then(|v| v.as_object()) {
        for (k, v) in existing_obj {
            if k != "defs" && k != "metainfo" {
                page.insert(k.clone(), v.clone());
            }
        }
    }
    BuiltPage {
        content: serde_json::Value::Object(page),
        missing_imgs,
        missing_icon_urls,
    }
}

fn wrap_page(json: &serde_json::Value) -> Result<String> {
    Ok(format!(
        "return [[\n{}\n]]",
        serde_json::to_string_pretty(json)?
    ))
}

fn set_img_url(content: &mut serde_json::Value, key: &str, url: String) {
    if let Some(imgs) = content
        .get_mut("metainfo")
        .and_then(|m| m.get_mut("imgs"))
        .and_then(|i| i.as_object_mut())
    {
        imgs.insert(key.to_string(), serde_json::json!(url));
    }
}

fn set_icon_url(content: &mut serde_json::Value, node: &str, url: String) {
    if let Some(def) = content
        .get_mut("defs")
        .and_then(|d| d.get_mut(node))
        .and_then(|d| d.as_object_mut())
    {
        def.insert("icon_url".into(), serde_json::json!(url));
    }
}

/// 背景图尺寸不符时给出的警告文案；尺寸正确或未知时返回 `None`。
fn bg_size_warning(file: &str, size: Option<(u32, u32)>) -> Option<String> {
    match size {
        Some(actual) if actual != GAME_BG_SIZE => Some(format!(
            "{} 实际 {}×{}，游戏原图应为 {}×{}；重传 images-sync 产物 \
             split/skilltree/{} 即可修复",
            file, actual.0, actual.1, GAME_BG_SIZE.0, GAME_BG_SIZE.1, file
        )),
        _ => None,
    }
}

/// 站内地址与模块里保存的背景 URL 不一致时返回新地址（文件被移动/重命名）。
fn background_url_update(stored: Option<&str>, current: Option<&str>) -> Option<String> {
    match (stored, current) {
        (Some(stored), Some(current)) if !stored.is_empty() && stored != current => {
            Some(current.to_string())
        }
        // 旧值为空（历史缺失）时由 check_background 负责回填。
        _ => None,
    }
}

/// 解析并校验角色背景图：回填/刷新 URL + 检查是否为游戏原图尺寸（625×384）。
///
/// 背景图单独查询，无论模块里是否已有 URL（历史 URL 可能指向被移动的
/// 文件）；解析成功会从 `missing_imgs` 移除该键。查询失败只告警不中断。
async fn check_background(
    client: &WikiClient,
    tree: &SkillTree,
    built: &mut BuiltPage,
    reporter: &dyn Reporter,
    page_title: &str,
) -> Vec<String> {
    let key = format!("{}_background", capitalize(&tree.character));
    let file = format!("{}.png", key);
    match client.get_files_info(&[file.as_str()]).await {
        Ok(infos) => {
            let Some(info) = infos
                .first()
                .filter(|info| !info.missing && info.url.is_some())
            else {
                return Vec::new();
            };
            if let Some(url) = info.url.clone() {
                let stored = built
                    .content
                    .get("metainfo")
                    .and_then(|m| m.get("imgs"))
                    .and_then(|i| i.get(&key))
                    .and_then(|v| v.as_str());
                if background_url_update(stored, Some(&url)).is_some() {
                    set_img_url(&mut built.content, &key, url);
                    reporter.log(format!("{}：背景 URL 已刷新为 {}", page_title, file));
                }
                built.missing_imgs.retain(|k| k != &key);
            }
            match bg_size_warning(&file, info.size) {
                Some(w) => {
                    reporter.log(format!("{}：警告：{}", page_title, w));
                    vec![w]
                }
                None => Vec::new(),
            }
        }
        Err(e) => {
            reporter.log(format!("警告：查询背景信息失败：{}", e));
            Vec::new()
        }
    }
}

/// 用维基图片 API 回填缺失的 `metainfo.imgs` URL 与 `defs[].icon_url`。
///
/// 只对旧页面里没有 URL 的键发起查询（≤50 个一批）；首字母大写、下划线
/// 等价空格等标题归一化由 [`WikiClient::get_files_info`] 处理。查询失败
/// 只告警不中断，缺失项留待手工上传后下次运行回填。
async fn fill_missing_urls(
    client: &WikiClient,
    tree: &SkillTree,
    built: &mut BuiltPage,
    reporter: &dyn Reporter,
    page_title: &str,
) {
    if !built.missing_imgs.is_empty() {
        // 背景键由 check_background 单独处理（回填/刷新 + 尺寸校验）。
        let (backgrounds, others): (Vec<String>, Vec<String>) = built
            .missing_imgs
            .iter()
            .cloned()
            .partition(|key| key.ends_with("_background"));
        let mut still_missing = backgrounds;
        if !others.is_empty() {
            let names: Vec<String> = others
                .iter()
                .map(|key| format!("{}.png", img_file_stem(key)))
                .collect();
            let refs: Vec<&str> = names.iter().map(String::as_str).collect();
            match client.get_files_info(&refs).await {
                Ok(infos) => {
                    let mut filled = 0usize;
                    for (key, info) in others.iter().zip(infos) {
                        match info.url {
                            Some(url) => {
                                set_img_url(&mut built.content, key, url);
                                filled += 1;
                            }
                            None => still_missing.push(key.clone()),
                        }
                    }
                    if filled > 0 {
                        reporter.log(format!("{}：已回填 {} 个图片 URL", page_title, filled));
                    }
                }
                Err(e) => {
                    reporter.log(format!("警告：查询图片信息失败：{}", e));
                    still_missing.extend(others);
                }
            }
        }
        built.missing_imgs = still_missing;
    }

    if !built.missing_icon_urls.is_empty() {
        let icon_of: std::collections::HashMap<&str, &str> = tree
            .nodes
            .iter()
            .filter_map(|n| n.icon.as_deref().map(|icon| (n.name.as_str(), icon)))
            .collect();
        let candidates: Vec<&String> = built
            .missing_icon_urls
            .iter()
            .filter(|name| icon_of.contains_key(name.as_str()))
            .collect();
        let names: Vec<String> = candidates
            .iter()
            .map(|name| format!("{}.png", icon_of[name.as_str()]))
            .collect();
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        match client.get_files_info(&refs).await {
            Ok(infos) => {
                let mut still_missing = Vec::new();
                let mut filled = 0usize;
                for (name, info) in candidates.into_iter().zip(infos) {
                    match info.url {
                        Some(url) => {
                            set_icon_url(&mut built.content, name, url);
                            filled += 1;
                        }
                        None => still_missing.push(name.clone()),
                    }
                }
                for name in &built.missing_icon_urls {
                    if !icon_of.contains_key(name.as_str()) {
                        still_missing.push(name.clone());
                    }
                }
                built.missing_icon_urls = still_missing;
                if filled > 0 {
                    reporter.log(format!("{}：已回填 {} 个图标 URL", page_title, filled));
                }
            }
            Err(e) => reporter.log(format!("警告：查询图标信息失败：{}", e)),
        }
    }
}

/// 单个角色的维护结果。
struct CharacterOutcome {
    status: String,
    added: usize,
    removed: usize,
    missing_imgs: Vec<String>,
    missing_icon_urls: Vec<String>,
    img_size_warnings: Vec<String>,
}

/// 跳过该角色（未做任何写维基动作）时的结果。
fn skipped_outcome(status: &str) -> CharacterOutcome {
    CharacterOutcome {
        status: status.to_string(),
        added: 0,
        removed: 0,
        missing_imgs: Vec::new(),
        missing_icon_urls: Vec::new(),
        img_size_warnings: Vec::new(),
    }
}

/// 单个角色的完整维护流程。
async fn maintain_character(
    client: &WikiClient,
    reporter: &dyn Reporter,
    mode: WriteMode,
    character: &str,
    snapshot: Option<&str>,
    strings: &std::collections::BTreeMap<String, String>,
    output_dir: Option<&PathBuf>,
) -> Result<CharacterOutcome> {
    let page_title = page_title(character);
    let content = read_game_file(snapshot, &format!("prefabs/skilltree_{}.lua", character))?;
    let tuning = super::dataset::load_tuning_numbers(snapshot)?;
    let tree = parse_skill_tree_with_tuning(&content, character, &tuning)?;

    // 只有确实“页面不存在”才走新建分支；限流/网络/认证错误一律中止该角色，
    // 绝不能当作缺失页面无 basetimestamp 全量覆盖。
    let page = match client.get_page(&page_title).await {
        Ok(p) => Some(p),
        Err(crate::error::Error::PageNotFound(_)) => None,
        Err(e) => {
            reporter.log(format!(
                "{}：拉取页面失败（{}），已跳过以免覆盖线上内容。",
                page_title, e
            ));
            return Ok(skipped_outcome("fetch_failed"));
        }
    };
    // 已有页面必须能解析：把解析失败静默当作新页面，会在下次写入时丢掉
    // 站内维护的 metainfo/icon_url。
    let existing = match parse_existing(page.as_ref().and_then(|p| p.content.as_deref())) {
        Ok(v) => v,
        Err(()) => {
            reporter.log(format!(
                "{}：页面内容不是预期的 return [[JSON]] 格式，已跳过（请手工核对），\
                 以免丢失站内 metainfo/icon_url。",
                page_title
            ));
            return Ok(skipped_outcome("page_content_unparsable"));
        }
    };

    let mut built = build_page_content(&tree, strings, existing.as_ref());
    fill_missing_urls(client, &tree, &mut built, reporter, &page_title).await;
    let img_size_warnings =
        check_background(client, &tree, &mut built, reporter, &page_title).await;
    let new_content = wrap_page(&built.content)?;

    if !built.missing_imgs.is_empty() {
        reporter.log(format!(
            "{}：待上传图片 {} 个：{}",
            page_title,
            built.missing_imgs.len(),
            built.missing_imgs.join(", ")
        ));
    }
    if !built.missing_icon_urls.is_empty() {
        reporter.log(format!(
            "{}：待补 icon_url 的技能 {} 个",
            page_title,
            built.missing_icon_urls.len()
        ));
    }

    if let Some(dir) = output_dir {
        std::fs::create_dir_all(dir)?;
        let file = dir.join(format!(
            "{}.lua",
            page_title.rsplit('/').next().unwrap_or(character)
        ));
        std::fs::write(&file, &new_content)?;
        reporter.log(format!("已写产物 {:?}", file));
    }

    let mut outcome = CharacterOutcome {
        status: String::new(),
        added: 0,
        removed: 0,
        missing_imgs: built.missing_imgs,
        missing_icon_urls: built.missing_icon_urls,
        img_size_warnings,
    };

    let old_content = page
        .as_ref()
        .and_then(|p| p.content.clone())
        .unwrap_or_default();
    if !old_content.trim().is_empty() && old_content.trim() == new_content.trim() {
        reporter.log(format!("{}：未检测到变化。", page_title));
        outcome.status = "no_changes".into();
        return Ok(outcome);
    }

    let (added, removed) = if old_content.trim().is_empty() {
        reporter.log(format!("{}：页面不存在，将创建。", page_title));
        (new_content.lines().count(), 0)
    } else {
        let diff = crate::diff_lines_preserve_whitespace(&old_content, &new_content);
        let (added, removed) = crate::count_diff_stats(&diff);
        reporter.diff(&page_title, &diff, added, removed);
        (added, removed)
    };
    outcome.added = added;
    outcome.removed = removed;

    let confirmed = if mode == WriteMode::Interactive {
        reporter.confirm(&format!("更新维基页面 {}？", page_title))
    } else {
        false
    };
    match decide_write(mode, confirmed) {
        WriteDecision::Skip(reason) => {
            reporter.log(format!("已跳过更新 {}（{}）。", page_title, reason));
            outcome.status = reason.to_string();
        }
        WriteDecision::Apply => {
            let basetimestamp = page.as_ref().and_then(|p| p.last_rev_timestamp.clone());
            let edit = client
                .edit_page(
                    &page_title,
                    &new_content,
                    Some("Update skilltree defs via dst-huiji-wiki tool"),
                    false,
                    basetimestamp.as_deref(),
                )
                .await?;
            reporter.log(format!(
                "{}：已写入（oldrev={:?} newrev={:?}）。",
                page_title, edit.oldrevid, edit.newrevid
            ));
            outcome.status = "updated".into();
        }
    }
    Ok(outcome)
}

/// 只有可能写入的模式才需要登录；dry-run 全程只读，允许匿名。
///
/// 登录态经 `WikiClient` 内部 Arc 共享，克隆时机不再影响可见性。
fn should_login(mode: WriteMode) -> bool {
    mode != WriteMode::DryRun
}

/// `skilltree-wiki` 任务入口：提取全部（或指定）角色的技能树并维护子页面。
pub async fn run_skilltree_wiki(
    character: Option<String>,
    output: Option<String>,
    snapshot: Option<String>,
    reporter: &dyn Reporter,
    mode: WriteMode,
) -> Result<serde_json::Value> {
    let dst_root = crate::platform::config::dst_root_str()?;
    let mut ctx = crate::DstContext::new(dst_root, snapshot.clone())?;

    // dry-run 只读不写，允许匿名；其余模式必须先登录（编辑时 assert=user）。
    if should_login(mode) {
        reporter.stage("登录维基");
        ctx.wiki_mut().login().await?;
    }
    let client = ctx.wiki().clone();

    let all_characters = super::dataset::list_skill_characters(&mut ctx)?;
    let characters: Vec<String> = match &character {
        Some(filter) => {
            let filter = filter.to_lowercase();
            let matched: Vec<String> = all_characters
                .iter()
                .filter(|c| c.to_lowercase().contains(&filter))
                .cloned()
                .collect();
            if matched.is_empty() {
                return Err(crate::Error::Config(format!(
                    "未找到匹配的角色技能树：{}（可用：{}）",
                    filter,
                    all_characters.join(", ")
                )));
            }
            matched
        }
        None => all_characters,
    };

    reporter.log(format!(
        "待处理角色（{}）：{}",
        characters.len(),
        characters.join(", ")
    ));

    let strings_data = load_skill_strings(snapshot.as_deref())?;
    let output_dir = output.as_ref().map(PathBuf::from);

    // 输出目录里额外放一份更新版渲染器，供手工上传到 零件:Skilltree.js。
    if let Some(dir) = output_dir.as_ref() {
        std::fs::create_dir_all(dir)?;
        let widget = dir.join("Skilltree.js");
        std::fs::write(&widget, SKILLTREE_WIDGET_JS)?;
        reporter.log(format!(
            "已写渲染器 {:?}（复制到维基 零件:Skilltree.js）",
            widget
        ));
    }

    let mut updated = 0usize;
    let mut no_changes = 0usize;
    let mut skipped = 0usize;
    let mut results = Vec::new();
    for ch in &characters {
        reporter.stage(&format!("模块:Skilltree/{}", ch));
        let outcome = maintain_character(
            &client,
            reporter,
            mode,
            ch,
            snapshot.as_deref(),
            &strings_data.by_ctxt,
            output_dir.as_ref(),
        )
        .await?;
        match outcome.status.as_str() {
            "updated" => updated += 1,
            "no_changes" => no_changes += 1,
            _ => skipped += 1,
        }
        results.push(serde_json::json!({
            "character": ch,
            "page": page_title(ch),
            "status": outcome.status,
            "added": outcome.added,
            "removed": outcome.removed,
            "missing_imgs": outcome.missing_imgs,
            "missing_icon_urls": outcome.missing_icon_urls,
            "img_size_warnings": outcome.img_size_warnings,
        }));
    }

    Ok(serde_json::json!({
        "updated": updated,
        "no_changes": no_changes,
        "skipped": skipped,
        "characters": results,
    }))
}

/// `skilltree-export` 任务入口：把技能树数据导出为本地 JSON 文件。
///
/// 纯本地操作，不读取/写入维基；需要 `DST__ROOT` 且已解压 scripts
/// （或指定快照目录）。每个角色输出一个 `<角色>.json`。
pub async fn run_skilltree_export(
    character: Option<String>,
    output: Option<PathBuf>,
    snapshot: Option<String>,
    reporter: &dyn Reporter,
) -> Result<serde_json::Value> {
    let all_characters = super::dataset::list_skill_characters_local(snapshot.as_deref())?;
    let characters: Vec<String> = match &character {
        Some(filter) => {
            let filter = filter.to_lowercase();
            let matched: Vec<String> = all_characters
                .iter()
                .filter(|c| c.to_lowercase().contains(&filter))
                .cloned()
                .collect();
            if matched.is_empty() {
                return Err(crate::Error::Config(format!(
                    "未找到匹配的角色技能树：{}（可用：{}）",
                    filter,
                    all_characters.join(", ")
                )));
            }
            matched
        }
        None => all_characters,
    };

    let output_dir = output.unwrap_or_else(|| PathBuf::from("output/skilltree"));
    std::fs::create_dir_all(&output_dir)?;
    reporter.log(format!(
        "待处理角色（{}）：{}",
        characters.len(),
        characters.join(", ")
    ));
    reporter.log(format!("输出目录：{}", output_dir.display()));

    let strings_data = load_skill_strings(snapshot.as_deref())?;
    let mut files = Vec::new();
    for ch in &characters {
        let data = super::dataset::load_skill_tree(snapshot.as_deref(), ch, &strings_data)?;
        let file = output_dir.join(format!("{}.json", ch));
        std::fs::write(&file, serde_json::to_string_pretty(&data)?)?;
        reporter.log(format!("已导出 {:?}", file));
        files.push(file.to_string_lossy().to_string());
    }

    Ok(serde_json::json!({
        "output": output_dir.to_string_lossy(),
        "characters": characters,
        "files": files,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_title_capitalizes() {
        assert_eq!(page_title("walter"), "模块:Skilltree/Walter");
        assert_eq!(page_title("wathgrithr"), "模块:Skilltree/Wathgrithr");
        assert_eq!(page_title("wx78"), "模块:Skilltree/Wx78");
    }

    #[test]
    fn test_parse_page_json() {
        let content = "return [[\n{\"defs\":{},\"metainfo\":{\"imgs\":{}}}\n]]";
        let parsed = parse_page_json(content).unwrap();
        assert!(parsed.get("defs").is_some());
        assert!(parse_page_json("bad content").is_none());
    }

    #[test]
    fn test_parse_existing_content_boundaries() {
        // 缺失 / 空内容 → 走新建分支。
        assert!(parse_existing(None).unwrap().is_none());
        assert!(parse_existing(Some("")).unwrap().is_none());
        assert!(parse_existing(Some("  \n")).unwrap().is_none());
        // 正常页面 → 解析成功。
        let page = "return [[\n{\"defs\":{}}\n]]";
        assert!(parse_existing(Some(page)).unwrap().is_some());
        // 非空但解析失败 → 跳过信号，绝不能当作新页面覆盖写。
        assert!(parse_existing(Some("<html>404</html>")).is_err());
        assert!(parse_existing(Some("return [[not json]]")).is_err());
    }

    fn sample_node(name: &str, icon: Option<&str>, infographic: bool) -> SkillNode {
        SkillNode {
            name: name.into(),
            x: -159.1,
            y: 14.5,
            group: Some("slingshotammo".into()),
            root: false,
            connects: vec![],
            icon: icon.map(str::to_string),
            lock: false,
            locks: vec!["walter_ammo_lock".into()],
            lock_open: None,
            tags: vec!["slingshotammo".into()],
            onactivate: true,
            ondeactivate: false,
            defaultfocus: false,
            infographic,
            forced_focus: None,
            button_decorations: false,
            decorations: vec![],
        }
    }

    fn sample_tree(nodes: Vec<SkillNode>) -> SkillTree {
        SkillTree {
            character: "walter".into(),
            nodes,
            background: None,
        }
    }

    #[test]
    fn test_build_page_content_preserves_metainfo_and_icon_url() {
        let tree = sample_tree(vec![sample_node(
            "walter_ammo_bag",
            Some("walter_ammo_bag"),
            false,
        )]);
        let existing: serde_json::Value = serde_json::json!({
            "defs": {
                "walter_ammo_bag": {
                    "pos": [-159.1, 14.5],
                    "icon_url": "https://example.com/Walter_ammo_bag.png"
                }
            },
            "metainfo": { "imgs": { "Frame": "https://example.com/Frame.png", "Extra_manual": "https://example.com/Extra.png" } }
        });
        let mut strings = std::collections::BTreeMap::new();
        strings.insert(
            "STRINGS.SKILLTREE.WALTER.WALTER_AMMO_BAG_TITLE".to_string(),
            "弹药囤积者".to_string(),
        );

        let built = build_page_content(&tree, &strings, Some(&existing));
        let page = &built.content;
        let def = &page["defs"]["walter_ammo_bag"];
        assert_eq!(def["icon_url"], "https://example.com/Walter_ammo_bag.png");
        assert_eq!(def["title"], "弹药囤积者");
        assert_eq!(def["onactivate"], true);
        // 已有 URL 保留；手工添加的额外键也保留。
        assert_eq!(
            page["metainfo"]["imgs"]["Frame"],
            "https://example.com/Frame.png"
        );
        assert_eq!(
            page["metainfo"]["imgs"]["Extra_manual"],
            "https://example.com/Extra.png"
        );
        // 清单里缺少的键写入空串并进入报告。
        assert_eq!(page["metainfo"]["imgs"]["Walter_background"], "");
        assert!(built
            .missing_imgs
            .contains(&"Walter_background".to_string()));
        assert!(built.missing_imgs.contains(&"Selected".to_string()));
        assert!(!built.missing_imgs.contains(&"Frame".to_string()));
        // 渲染几何随 metainfo 输出，默认 tint 开启。
        assert_eq!(page["metainfo"]["render"]["bg"]["size"][0], 521);
        assert_eq!(page["metainfo"]["render"]["bg"]["tint"], true);
        // 技能图标已上传，无缺失 icon_url。
        assert!(built.missing_icon_urls.is_empty());

        // 新页面：icon_url 缺失计入报告。
        let fresh = build_page_content(&tree, &strings, None);
        assert!(fresh
            .missing_icon_urls
            .contains(&"walter_ammo_bag".to_string()));
        assert!(fresh.content["defs"]["walter_ammo_bag"]
            .get("icon_url")
            .is_none());
    }

    #[test]
    fn test_expected_img_keys_include_infographic_and_decorations() {
        let mut tree = sample_tree(vec![sample_node(
            "wortox_scales",
            Some("wortox_scales"),
            true,
        )]);
        tree.character = "wortox".into();
        tree.nodes[0].decorations = vec![crate::parser::skilltree::SkillDecoration {
            img: "winona_background1".into(),
            pos: [-3.0, -68.0],
            size: Some([520.0, 90.0]),
            scale: None,
        }];
        tree.background = Some(crate::parser::skilltree::BackgroundSettings {
            tint_bright: Some(false),
        });
        let keys = expected_img_keys(&tree);
        assert!(keys.contains(&"Wortox_background".to_string()));
        assert!(keys.contains(&"Frame_infographic".to_string()));
        assert!(keys.contains(&"Infographic_off".to_string()));
        assert!(keys.contains(&"Winona_background1".to_string()));

        let built = build_page_content(&tree, &std::collections::BTreeMap::new(), None);
        assert_eq!(built.content["metainfo"]["render"]["bg"]["tint"], false);
        assert_eq!(
            built.content["defs"]["wortox_scales"]["decorations"][0]["img"],
            "winona_background1"
        );
    }

    #[test]
    fn test_should_login_only_for_write_modes() {
        assert!(!should_login(WriteMode::DryRun));
        assert!(should_login(WriteMode::AutoConfirm));
        assert!(should_login(WriteMode::Interactive));
    }

    #[test]
    fn test_img_file_stem_alias() {
        assert_eq!(img_file_stem("Winona_background1"), "Winona_background1");
        assert_eq!(img_file_stem("Infographic_off"), "Infographic_off");
        assert_eq!(
            img_file_stem("Button_carny_long_normal"),
            "Button_long_normal"
        );
        assert_eq!(img_file_stem("Button_carny_long_down"), "Button_long_down");
    }

    #[test]
    fn test_background_url_update() {
        // 地址变化（文件被移动/重命名）时返回新地址。
        assert_eq!(
            background_url_update(Some("https://old/a.png"), Some("https://new/a.png")).as_deref(),
            Some("https://new/a.png")
        );
        // 一致、旧值为空或站内缺失时都不动。
        assert!(background_url_update(Some("https://x/a.png"), Some("https://x/a.png")).is_none());
        assert!(background_url_update(Some(""), Some("https://x/a.png")).is_none());
        assert!(background_url_update(None, Some("https://x/a.png")).is_none());
        assert!(background_url_update(Some("https://old/a.png"), None).is_none());
    }

    #[test]
    fn test_bg_size_warning() {
        // 游戏原图与未知尺寸都不告警。
        assert!(bg_size_warning("Wilson_background.png", Some((625, 384))).is_none());
        assert!(bg_size_warning("Wilson_background.png", None).is_none());
        // 站内历史拉伸放大版会告警并带实际尺寸。
        let w = bg_size_warning("Wilson_background.png", Some((852, 756))).unwrap();
        assert!(w.contains("852×756"), "{}", w);
        assert!(w.contains("625×384"), "{}", w);
        assert!(w.contains("Wilson_background.png"), "{}", w);
    }
}
