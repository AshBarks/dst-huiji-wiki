//! 由图标文件清单 + 名称表构建 `IconMeta`（保留未变标题的 wiki 状态）。

use crate::scripts_sync::images::history::now_ms;
use std::collections::BTreeMap;

use super::names::{CraftingKeys, NameMaps, SkilltreeNames};
use super::overrides::IconTitleOverrides;
use super::title::{effective_title, file_stem_key, invalid_note};
use super::{IconGroup, IconMeta, IconMetaEntry};

/// 由当前图标文件清单构建元数据；`previous` 中生效标题未变的条目的 wiki
/// 状态会保留（避免每次 images-sync 重查全部标题）。
///
/// `files` 为 `(文件名, 来源标识)` 对（来源见 `images::icons::ICON_SOURCES`）；
/// 命名按来源分支：`crafting` 走 [`CraftingKeys::display_name`]，
/// `skilltree` 走 [`SkilltreeNames::display_name`]，其余查 `STRINGS.NAMES`。
pub fn build_meta(
    build: &str,
    files: impl IntoIterator<Item = (String, String)>,
    names: &NameMaps,
    crafting: &CraftingKeys,
    skilltree: &SkilltreeNames,
    overrides: &IconTitleOverrides,
    previous: &IconMeta,
) -> IconMeta {
    let mut icons = BTreeMap::new();
    for (file, source) in files {
        let (name_en, name_zh) = match source.as_str() {
            "crafting" => crafting
                .display_name(names, &file)
                .map(|(en, zh)| (Some(en), zh))
                .unwrap_or_default(),
            "skilltree" => skilltree
                .display_name(&file)
                .map(|(en, zh)| (Some(en), zh))
                .unwrap_or_default(),
            _ => {
                let key = file_stem_key(&file);
                (
                    names.names.en.get(&key).cloned(),
                    names.names.zh.get(&key).cloned(),
                )
            }
        };
        let (title, title_source) =
            effective_title(&file, name_en.as_deref(), overrides, Some(source.as_str()));
        if name_en.is_none() && title.is_none() {
            continue;
        }
        let uploadable = title.is_some();
        let note = match (&title, name_en.as_deref()) {
            (None, Some(name_en)) => Some(invalid_note(name_en)),
            _ => None,
        };
        let wiki = previous
            .icons
            .get(&file)
            .filter(|old| old.title == title)
            .and_then(|old| old.wiki.clone());
        let group = match source.as_str() {
            "crafting" => crafting.group(&file),
            "skilltree" => Some(IconGroup::Skilltree {
                character: skilltree.character(&file),
            }),
            _ => None,
        };
        icons.insert(
            file,
            IconMetaEntry {
                name_en,
                name_zh,
                source: Some(source),
                group,
                title,
                title_source,
                uploadable,
                note,
                wiki,
            },
        );
    }
    IconMeta {
        build: Some(build.to_string()),
        generated_at: Some(now_ms()),
        icons,
    }
}
