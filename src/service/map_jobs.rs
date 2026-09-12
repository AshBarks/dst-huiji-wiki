//! PO/配方映射作业（parse-po / map-names / map-recipes）。

use super::output::finish_json_output;
use super::path_str;
use crate::error::{Error, Result};
use crate::mapping::{compare_and_report, WikiDataConverter, WikiMapper};
use crate::models::PoEntry;
use crate::parser::RecipeParser;
use crate::platform::progress::Reporter;
use std::path::PathBuf;

pub(super) async fn run_parse_po(
    input: &str,
    output: Option<PathBuf>,
    category: Option<String>,
    reporter: &dyn Reporter,
) -> Result<serde_json::Value> {
    reporter.stage("解析 PO 文件");
    let po_file = crate::parser::PoParser::parse_from_file(path_str(input)?)?;
    let entries = if let Some(cat) = category {
        po_file.filter_by_category(&cat)
    } else {
        po_file.entries.iter().collect::<Vec<_>>()
    };

    reporter.log(format!("共 {} 条目", entries.len()));

    if let Some(output_path) = output {
        crate::platform::fs::write_json_atomic(&output_path, &entries)?;
        reporter.log(format!("已写入 {} 条目到 {:?}", entries.len(), output_path));
        Ok(serde_json::json!({ "entries": entries.len(), "output": output_path }))
    } else {
        for entry in entries.iter().take(10) {
            reporter.log(format!("{:?}", entry));
        }
        reporter.log(format!("... 共 {} 条目", entries.len()));
        Ok(serde_json::json!({ "entries": entries.len() }))
    }
}

pub(super) async fn run_map_names(
    input: &str,
    output: Option<PathBuf>,
    compare: Option<String>,
    merge: bool,
    version: Option<String>,
    reporter: &dyn Reporter,
) -> Result<serde_json::Value> {
    reporter.stage("解析 PO 文件");
    let po_file = crate::parser::PoParser::parse_from_file(path_str(input)?)?;
    let names_entries: Vec<PoEntry> = po_file
        .entries
        .iter()
        .filter(|e| {
            e.msgctxt
                .as_ref()
                .map(|ctx: &String| ctx.starts_with("STRINGS.NAMES."))
                .unwrap_or(false)
        })
        .cloned()
        .collect();

    reporter.log(format!("找到 {} 条 NAMES 条目", names_entries.len()));

    let converter = WikiDataConverter::new();
    let version_str = version.as_deref().unwrap_or("unknown");
    let sources = format!("Extract data from patch {}", version_str);
    let description = Some(serde_json::json!({
        "zh": "饥荒联机版的实体代码、中英文名及图片对照表"
    }));

    reporter.stage("生成维基数据");
    let wiki_data = if merge {
        let compare_path = compare
            .clone()
            .ok_or_else(|| Error::Config("--merge requires --compare".to_string()))?;
        let historical_json = std::fs::read_to_string(&compare_path)?;
        let historical_data = WikiDataConverter::parse_wiki_json(&historical_json)?;
        converter.convert_with_history(&names_entries, &sources, &historical_data, description)
    } else {
        converter.convert_to_wiki_json(&names_entries, &sources, description)
    };

    if let Some(compare_path) = &compare {
        if !merge {
            let historical_json = std::fs::read_to_string(compare_path)?;
            let historical_data = WikiDataConverter::parse_wiki_json(&historical_json)?;
            reporter.log(compare_and_report(&wiki_data, &historical_data));
        }
    }

    finish_json_output(&wiki_data, output, reporter)
}

pub(super) async fn run_map_recipes(
    input: &str,
    output: Option<PathBuf>,
    compare: Option<String>,
    merge: bool,
    po_file: Option<String>,
    version: Option<String>,
    reporter: &dyn Reporter,
) -> Result<serde_json::Value> {
    reporter.stage("解析配方 Lua");
    let lua_content = std::fs::read_to_string(input)?;
    let mut parser = RecipeParser::new();
    let recipes = parser.parse(&lua_content, Some(input))?;

    reporter.log(format!("找到 {} 个配方", recipes.len()));

    let converter = if let Some(po_path) = &po_file {
        match crate::parser::PoParser::parse_from_file(po_path) {
            Ok(po_data) => {
                reporter.log(format!(
                    "载入 {} 条 PO 条目用于描述查询",
                    po_data.entries.len()
                ));
                WikiDataConverter::with_po_entries(po_data.entries.clone())
            }
            Err(e) => {
                reporter.log(format!("警告：PO 文件加载失败：{}", e));
                WikiDataConverter::new()
            }
        }
    } else {
        WikiDataConverter::new()
    };

    let version_str = version.as_deref().unwrap_or("unknown");
    let sources = format!("Extract data from patch {}", version_str);
    let description = Some(serde_json::json!({
        "zh": "饥荒联机版的合成配方列表"
    }));

    reporter.stage("生成维基数据");
    let wiki_data = if merge {
        let compare_path = compare
            .clone()
            .ok_or_else(|| Error::Config("--merge requires --compare".to_string()))?;
        let historical_json = std::fs::read_to_string(&compare_path)?;
        let historical_data = WikiDataConverter::parse_wiki_json(&historical_json)?;
        let mut data = converter.convert_recipes(&recipes, &sources, description);
        crate::models::Recipe::merge_with_history(&mut data, &historical_data);
        data
    } else {
        converter.convert_recipes(&recipes, &sources, description)
    };

    if let Some(compare_path) = &compare {
        if !merge {
            let historical_json = std::fs::read_to_string(compare_path)?;
            let historical_data = WikiDataConverter::parse_wiki_json(&historical_json)?;
            reporter.log(compare_and_report(&wiki_data, &historical_data));
        }
    }

    finish_json_output(&wiki_data, output, reporter)
}
