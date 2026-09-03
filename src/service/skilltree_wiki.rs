//! 模块:Skilltree/<Character> 子页面维护。
//!
//! 用技能树解析器把游戏 `skilltree_<char>.lua` 提取成子页面的 `defs` JSON
//! （与灰机维基 `模块:Skilltree` 前端零件:Skilltree.js 消费的格式一致）。
//! 更新已有子页面时保留页内 `metainfo`（以及旧 defs 里的 `icon_url`，
//! 那是维基站内上传图片后才有的地址，本地无法生成）。

use super::dataset::{load_skill_strings, read_game_file};
use super::{decide_write, WriteDecision, WriteMode};
use crate::error::Result;
use crate::parser::skilltree::{parse_skill_tree, SkillNode};
use crate::service::progress::Reporter;
use crate::wiki::WikiClient;
use std::path::PathBuf;

/// 一级命名空间内的子页面标题，例如 `模块:Skilltree/Walter`。
fn page_title(character: &str) -> String {
    let mut chars = character.chars();
    let head = chars.next().map(|c| c.to_uppercase().to_string());
    match head {
        Some(h) => format!("模块:Skilltree/{}{}", h, chars.as_str()),
        None => "模块:Skilltree".to_string(),
    }
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

/// 用新提取的 defs 组装子页面内容；已有页面保留 metainfo 与 icon_url。
fn build_page_content(
    nodes: &[SkillNode],
    strings: &std::collections::BTreeMap<String, String>,
    character: &str,
    existing: Option<&serde_json::Value>,
) -> serde_json::Value {
    let upper_char = character.to_uppercase();
    let prefix = format!("STRINGS.SKILLTREE.{}.", upper_char);

    let old_defs = existing
        .and_then(|v| v.get("defs"))
        .and_then(|d| d.as_object())
        .cloned()
        .unwrap_or_default();

    let mut defs = serde_json::Map::new();
    for node in nodes {
        let title_key = format!("{}{}_TITLE", prefix, node.name.to_uppercase());
        let desc_key = format!("{}{}_DESC", prefix, node.name.to_uppercase());
        let mut def = def_to_json(node, strings.get(&title_key), strings.get(&desc_key));
        // 维基侧维护的 icon_url（站内图片地址）原样保留。
        if let Some(icon_url) = old_defs.get(&node.name).and_then(|d| d.get("icon_url")) {
            if let Some(obj) = def.as_object_mut() {
                obj.insert("icon_url".into(), icon_url.clone());
            }
        }
        defs.insert(node.name.clone(), def);
    }

    let mut page = serde_json::Map::new();
    page.insert("defs".into(), serde_json::Value::Object(defs));
    // 保留旧页面的其余顶层键（metainfo.imgs 等）；新页面给空 imgs。
    if let Some(existing_obj) = existing.and_then(|v| v.as_object()) {
        for (k, v) in existing_obj {
            if k != "defs" {
                page.insert(k.clone(), v.clone());
            }
        }
    } else {
        page.insert("metainfo".into(), serde_json::json!({ "imgs": {} }));
    }
    serde_json::Value::Object(page)
}

fn wrap_page(json: &serde_json::Value) -> Result<String> {
    Ok(format!(
        "return [[\n{}\n]]",
        serde_json::to_string_pretty(json)?
    ))
}

/// 单个角色的完整维护流程，返回 ("status", added, removed)。
async fn maintain_character(
    client: &WikiClient,
    reporter: &dyn Reporter,
    mode: WriteMode,
    character: &str,
    strings: &std::collections::BTreeMap<String, String>,
    output_dir: Option<&PathBuf>,
) -> Result<(String, usize, usize)> {
    let page_title = page_title(character);
    let content = read_game_file(None, &format!("prefabs/skilltree_{}.lua", character))?;
    let tree = parse_skill_tree(&content, character)?;

    let page = client.get_page(&page_title).await.ok();
    let existing = page
        .as_ref()
        .and_then(|p| p.content.as_deref())
        .and_then(parse_page_json);

    let new_json = build_page_content(&tree.nodes, strings, character, existing.as_ref());
    let new_content = wrap_page(&new_json)?;

    if let Some(dir) = output_dir {
        std::fs::create_dir_all(dir)?;
        let file = dir.join(format!(
            "{}.lua",
            page_title.rsplit('/').next().unwrap_or(character)
        ));
        std::fs::write(&file, &new_content)?;
        reporter.log(format!("已写产物 {:?}", file));
    }

    let old_content = page
        .as_ref()
        .and_then(|p| p.content.clone())
        .unwrap_or_default();
    if !old_content.trim().is_empty() && old_content.trim() == new_content.trim() {
        reporter.log(format!("{}：未检测到变化。", page_title));
        return Ok(("no_changes".into(), 0, 0));
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

    let confirmed = if mode == WriteMode::Interactive {
        reporter.confirm(&format!("更新维基页面 {}？", page_title))
    } else {
        false
    };
    match decide_write(mode, confirmed) {
        WriteDecision::Skip(reason) => {
            reporter.log(format!("已跳过更新 {}（{}）。", page_title, reason));
            Ok((reason.to_string(), added, removed))
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
            Ok(("updated".into(), added, removed))
        }
    }
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

    let mut updated = 0usize;
    let mut no_changes = 0usize;
    let mut skipped = 0usize;
    let mut results = Vec::new();
    for ch in &characters {
        reporter.stage(&format!("模块:Skilltree/{}", ch));
        let (status, added, removed) = maintain_character(
            &client,
            reporter,
            mode,
            ch,
            &strings_data.by_ctxt,
            output_dir.as_ref(),
        )
        .await?;
        match status.as_str() {
            "updated" => updated += 1,
            "no_changes" => no_changes += 1,
            _ => skipped += 1,
        }
        results.push(serde_json::json!({
            "character": ch,
            "page": page_title(ch),
            "status": status,
            "added": added,
            "removed": removed,
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
    fn test_build_page_content_preserves_metainfo_and_icon_url() {
        let nodes: Vec<SkillNode> = vec![SkillNode {
            name: "walter_ammo_bag".into(),
            x: -159.1,
            y: 14.5,
            group: Some("slingshotammo".into()),
            root: false,
            connects: vec![],
            icon: Some("walter_ammo_bag".into()),
            lock: false,
            locks: vec!["walter_ammo_lock".into()],
            lock_open: None,
            tags: vec!["slingshotammo".into()],
            onactivate: true,
            ondeactivate: false,
            defaultfocus: false,
            infographic: false,
            forced_focus: None,
            button_decorations: false,
        }];
        let existing: serde_json::Value = serde_json::json!({
            "defs": {
                "walter_ammo_bag": {
                    "pos": [-159.1, 14.5],
                    "icon_url": "https://example.com/Walter_ammo_bag.png"
                }
            },
            "metainfo": { "imgs": { "Frame": "https://example.com/Frame.png" } }
        });
        let mut strings = std::collections::BTreeMap::new();
        strings.insert(
            "STRINGS.SKILLTREE.WALTER.WALTER_AMMO_BAG_TITLE".to_string(),
            "弹药囤积者".to_string(),
        );

        let page = build_page_content(&nodes, &strings, "walter", Some(&existing));
        let def = &page["defs"]["walter_ammo_bag"];
        assert_eq!(def["icon_url"], "https://example.com/Walter_ammo_bag.png");
        assert_eq!(def["title"], "弹药囤积者");
        assert_eq!(def["onactivate"], true);
        assert!(page["metainfo"]["imgs"]["Frame"].is_string());

        // 新页面没有旧内容：给空 metainfo，无 icon_url。
        let fresh = build_page_content(&nodes, &strings, "walter", None);
        assert!(fresh["metainfo"]["imgs"].as_object().unwrap().is_empty());
        assert!(fresh["defs"]["walter_ammo_bag"].get("icon_url").is_none());
    }
}
