//! 模块:Skilltree/<Character> 子页面维护。
//!
//! 用技能树解析器把游戏 `skilltree_<char>.lua` 提取成子页面的 `defs` JSON
//! （与灰机维基 `模块:Skilltree` 前端零件:Skilltree.js 消费的格式一致）。
//! 更新已有子页面时保留页内 `metainfo`（以及旧 defs 里的 `icon_url`，
//! 那是维基站内上传图片后才有的地址，本地无法生成）。
//!
//! `metainfo` 里除 `imgs` 外还写入 `render`（游戏部件几何：背景矩形、
//! 节点偏移、XP 位置、背景 tint），渲染器据此布局而无需硬编码常量；
//! `imgs` 会与角色所需图片清单合并（缺失键留空并计入报告），方便手工
//! 上传后回填。`--output` 目录同时写出 `Skilltree.js`（更新版渲染器，
//! 复制到维基 `零件:Skilltree.js`）。

use super::dataset::{load_skill_strings, read_game_file};
use super::{decide_write, WriteDecision, WriteMode};
use crate::error::Result;
use crate::parser::skilltree::{parse_skill_tree_with_tuning, SkillNode, SkillTree};
use crate::service::progress::Reporter;
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

/// 单个角色的维护结果。
struct CharacterOutcome {
    status: String,
    added: usize,
    removed: usize,
    missing_imgs: Vec<String>,
    missing_icon_urls: Vec<String>,
}

/// 单个角色的完整维护流程。
async fn maintain_character(
    client: &WikiClient,
    reporter: &dyn Reporter,
    mode: WriteMode,
    character: &str,
    strings: &std::collections::BTreeMap<String, String>,
    output_dir: Option<&PathBuf>,
) -> Result<CharacterOutcome> {
    let page_title = page_title(character);
    let content = read_game_file(None, &format!("prefabs/skilltree_{}.lua", character))?;
    let tuning = super::dataset::load_tuning_numbers(None)?;
    let tree = parse_skill_tree_with_tuning(&content, character, &tuning)?;

    let page = client.get_page(&page_title).await.ok();
    let existing = page
        .as_ref()
        .and_then(|p| p.content.as_deref())
        .and_then(parse_page_json);

    let built = build_page_content(&tree, strings, existing.as_ref());
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

/// `skilltree-wiki` 任务入口：提取全部（或指定）角色的技能树并维护子页面。
pub async fn run_skilltree_wiki(
    character: Option<String>,
    output: Option<String>,
    snapshot: Option<String>,
    reporter: &dyn Reporter,
    mode: WriteMode,
) -> Result<serde_json::Value> {
    let dst_root = std::env::var("DST__ROOT")
        .map_err(|e| crate::Error::EnvVarNotFound(format!("DST__ROOT: {}", e)))?;
    let mut ctx = crate::DstContext::new(dst_root, snapshot.clone())?;
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
}
