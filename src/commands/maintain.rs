use super::Commands;
use dst_huiji_wiki::copyclip::{process_copyclip, CopyClipProcessor};
use dst_huiji_wiki::diff_lines;
use dst_huiji_wiki::mapping::{compare_and_report, WikiDataConverter, WikiMapper};
use dst_huiji_wiki::models::PoEntry;
use dst_huiji_wiki::parser::{extract_field_assignment_range, PoParser, RecipeParser};
use dst_huiji_wiki::wiki::WikiClient;
use dst_huiji_wiki::{DstContext, Error, Result, TechReport};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

pub async fn run(args: Commands) -> Result<()> {
    match args {
        Commands::ParsePo {
            input,
            output,
            category,
        } => handle_parse_po(input, output, category),
        Commands::MapNames {
            input,
            output,
            compare,
            merge,
            version,
        } => handle_map_names(input, output, compare, merge, version),
        Commands::MapRecipes {
            input,
            output,
            compare,
            merge,
            po_file,
            version,
        } => handle_map_recipes(input, output, compare, merge, po_file, version),
        Commands::MaintainItemTable { output } => handle_maintain_item_table(output).await,
        Commands::MaintainDSTRecipes { output } => handle_maintain_dst_recipes(output).await,
        Commands::MaintainCopyClip { r#type, output } => {
            handle_maintain_copyclip(r#type.as_deref(), output).await
        }
    }
}

fn handle_parse_po(input: PathBuf, output: Option<PathBuf>, category: Option<String>) -> Result<()> {
    let po_file = PoParser::parse_from_file(input.to_str().unwrap())?;
    let entries = if let Some(cat) = category {
        po_file.filter_by_category(&cat)
    } else {
        po_file.entries.iter().collect()
    };

    if let Some(output_path) = output {
        let json = serde_json::to_string_pretty(&entries).unwrap();
        std::fs::write(&output_path, json).unwrap();
        println!("Written {} entries to {:?}", entries.len(), output_path);
    } else {
        for entry in entries.iter().take(10) {
            println!("{:?}", entry);
        }
        println!("... ({} total entries)", entries.len());
    }

    Ok(())
}

fn handle_map_names(
    input: PathBuf,
    output: Option<PathBuf>,
    compare: Option<PathBuf>,
    merge: bool,
    version: Option<String>,
) -> Result<()> {
    let po_file = PoParser::parse_from_file(input.to_str().unwrap())?;
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

    println!("Found {} NAMES entries", names_entries.len());

    let converter = WikiDataConverter::new();
    let version_str = version.as_deref().unwrap_or("unknown");
    let sources = format!("Extract data from DST version {}", version_str);

    let wiki_data = if merge {
        let compare_path = compare
            .as_ref()
            .ok_or_else(|| Error::Config("--merge requires --compare".to_string()))?;
        let historical_json = std::fs::read_to_string(compare_path)?;
        let historical_data = WikiDataConverter::parse_wiki_json(&historical_json)?;
        converter.convert_with_history(&names_entries, &sources, &historical_data)
    } else {
        converter.convert_to_wiki_json(&names_entries, &sources)
    };

    if let Some(compare_path) = &compare {
        if !merge {
            let historical_json = std::fs::read_to_string(compare_path)?;
            let historical_data = WikiDataConverter::parse_wiki_json(&historical_json)?;
            println!("\n{}", compare_and_report(&wiki_data, &historical_data));
        }
    }

    if let Some(output_path) = output {
        let json = WikiDataConverter::to_json_string(&wiki_data)?;
        std::fs::write(&output_path, json)?;
        println!("Written {} records to {:?}", wiki_data.data.len(), output_path);
    } else {
        println!("\nFirst 5 records:");
        for (i, record) in wiki_data.data.iter().take(5).enumerate() {
            println!("{}: {:?}", i + 1, record);
        }
    }

    Ok(())
}

fn handle_map_recipes(
    input: PathBuf,
    output: Option<PathBuf>,
    compare: Option<PathBuf>,
    merge: bool,
    po_file: Option<PathBuf>,
    version: Option<String>,
) -> Result<()> {
    let lua_content = std::fs::read_to_string(&input)?;
    let mut parser = RecipeParser::new();
    let recipes = parser.parse(&lua_content, input.to_str())?;

    println!("Found {} recipes", recipes.len());

    let converter = if let Some(po_path) = &po_file {
        match PoParser::parse_from_file(po_path.to_str().unwrap()) {
            Ok(po_data) => {
                println!("Loaded {} PO entries for desc lookup", po_data.entries.len());
                WikiDataConverter::with_po_entries(po_data.entries.clone())
            }
            Err(e) => {
                eprintln!("Warning: Failed to load PO file: {}", e);
                WikiDataConverter::new()
            }
        }
    } else {
        WikiDataConverter::new()
    };

    let version_str = version.as_deref().unwrap_or("unknown");
    let sources = format!("Extract data from DST version {}", version_str);

    let wiki_data = if merge {
        let compare_path = compare
            .as_ref()
            .ok_or_else(|| Error::Config("--merge requires --compare".to_string()))?;
        let historical_json = std::fs::read_to_string(compare_path)?;
        let historical_data = WikiDataConverter::parse_wiki_json(&historical_json)?;
        let mut data = converter.convert_recipes(&recipes, &sources);
        dst_huiji_wiki::models::Recipe::merge_with_history(&mut data, &historical_data);
        data
    } else {
        converter.convert_recipes(&recipes, &sources)
    };

    if let Some(compare_path) = &compare {
        if !merge {
            let historical_json = std::fs::read_to_string(compare_path)?;
            let historical_data = WikiDataConverter::parse_wiki_json(&historical_json)?;
            println!("\n{}", compare_and_report(&wiki_data, &historical_data));
        }
    }

    if let Some(output_path) = output {
        let json = WikiDataConverter::to_json_string(&wiki_data)?;
        std::fs::write(&output_path, json)?;
        println!("Written {} records to {:?}", wiki_data.data.len(), output_path);
    } else {
        println!("\nFirst 5 records:");
        for (i, record) in wiki_data.data.iter().take(5).enumerate() {
            println!("{}: {:?}", i + 1, record);
        }
    }

    Ok(())
}

async fn handle_maintain_item_table(output: Option<PathBuf>) -> Result<()> {
    let mut ctx = DstContext::from_env()?;
    println!("DST version: {}", ctx.version);

    println!("Logging in to wiki...");
    ctx.client.login().await?;

    let po_file = ctx.parse_po_file("scripts/languages/chinese_s.po")?;
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

    println!("Found {} NAMES entries", names_entries.len());

    let converter = WikiDataConverter::new();

    println!("Fetching historical data from wiki...");
    let page_title = "Data:ItemTable.tabx";
    let historical_data = match ctx.client.get_json_data(page_title).await {
        Ok(historical_json) => {
            Some(WikiDataConverter::parse_wiki_json(&historical_json.to_string())?)
        }
        Err(e) => {
            println!("Warning: Failed to fetch historical data from wiki: {}", e);
            println!("Proceeding without historical data...");
            None
        }
    };

    let sources = ctx.sources();
    let mut wiki_data = converter.convert_to_wiki_json(&names_entries, &sources);

    if let Some(ref historical) = historical_data {
        PoEntry::merge_with_history(&mut wiki_data, historical);
        println!("\n{}", compare_and_report(&wiki_data, historical));
    }

    output_json_result_with_update(
        &ctx.client,
        page_title,
        &wiki_data,
        output,
    )
    .await
}

async fn handle_maintain_dst_recipes(output: Option<PathBuf>) -> Result<()> {
    let mut ctx = DstContext::from_env()?;
    println!("DST version: {}", ctx.version);

    println!("Logging in to wiki...");
    ctx.client.login().await?;

    let recipes_string = ctx.read_zip_file("scripts/recipes.lua")?;

    println!("Parsing recipes.lua...");
    let mut parser = RecipeParser::new();
    let recipes = parser.parse(&recipes_string, Some("scripts/recipes.lua"))?;

    println!("Found {} recipes", recipes.len());

    println!("\nFetching Tech data from wiki for comparison...");
    let mut tech_report = TechReport::from_recipes(&recipes);

    match ctx.client.get_page("模块:RenderRecsByIngre/Data").await {
        Ok(page) => {
            if let Some(content) = &page.content {
                tech_report.compare_with_wiki(content);
                println!("\n{}", tech_report.generate_report());
            } else {
                println!("Warning: Wiki page has no content");
            }
        }
        Err(e) => {
            println!("Warning: Failed to fetch Tech data from wiki: {}", e);
            println!("Proceeding without Tech comparison...");
        }
    }

    println!("\nParsing chinese_s.po for desc lookup...");
    let po_file = ctx.parse_po_file("scripts/languages/chinese_s.po")?;
    println!("Loaded {} PO entries for desc lookup", po_file.entries.len());

    let converter = WikiDataConverter::with_po_entries(po_file.entries.clone());

    println!("Fetching historical data from wiki...");
    let page_title = "Data:DSTRecipes.tabx";
    let historical_data = match ctx.client.get_json_data(page_title).await {
        Ok(historical_json) => {
            Some(WikiDataConverter::parse_wiki_json(&historical_json.to_string())?)
        }
        Err(e) => {
            println!("Warning: Failed to fetch historical data from wiki: {}", e);
            println!("Proceeding without historical data...");
            None
        }
    };

    let sources = ctx.sources();
    let mut wiki_data = converter.convert_recipes(&recipes, &sources);

    if let Some(ref historical) = historical_data {
        dst_huiji_wiki::models::Recipe::merge_with_history(&mut wiki_data, historical);
        println!("\n{}", compare_and_report(&wiki_data, historical));
    }

    output_json_result_with_update(
        &ctx.client,
        page_title,
        &wiki_data,
        output,
    )
    .await
}

async fn handle_maintain_copyclip(r#type: Option<&str>, output: Option<PathBuf>) -> Result<()> {
    let mut ctx = DstContext::from_env()?;
    println!("DST version: {}", ctx.version);

    println!("Logging in to wiki...");
    ctx.client.login().await?;

    let types_to_run = if let Some(t) = r#type {
        vec![t.to_lowercase()]
    } else {
        vec![
            "recipe_builder_tag_lookup".to_string(),
            "tech".to_string(),
            "crafting_filters".to_string(),
            "crafting_names".to_string(),
        ]
    };

    for t in types_to_run {
        println!("\n========== Running: {} ==========\n", t);
        match t.as_str() {
            "recipe_builder_tag_lookup" | "rbtl" => {
                maintain_recipe_builder_tag_lookup(&mut ctx, output.clone()).await?;
            }
            "tech" => {
                maintain_tech(&mut ctx, output.clone()).await?;
            }
            "crafting_filters" | "filters" => {
                maintain_crafting_filters(&mut ctx, output.clone()).await?;
            }
            "crafting_names" | "names" => {
                maintain_crafting_names(&mut ctx, output.clone()).await?;
            }
            _ => {
                eprintln!("Unknown type: {}. Valid types are:", t);
                eprintln!("  - recipe_builder_tag_lookup (or rbtl)");
                eprintln!("  - tech");
                eprintln!("  - crafting_filters (or filters)");
                eprintln!("  - crafting_names (or names)");
                std::process::exit(1);
            }
        }
    }

    Ok(())
}

async fn maintain_recipe_builder_tag_lookup(
    ctx: &mut DstContext,
    output: Option<PathBuf>,
) -> Result<()> {
    let debugcommands_string = ctx.read_zip_file("scripts/debugcommands.lua")?;

    println!("Fetching wiki page content...");
    let page_title = "模块:Constants/RecipeBuilderTagLookup";
    let page = ctx.client.get_page(page_title).await?;

    let target_content = page
        .content
        .ok_or_else(|| Error::WikiApi("Wiki page has no content".to_string()))?;

    println!("Extracting RECIPE_BUILDER_TAG_LOOKUP from debugcommands.lua...");
    let result = process_copyclip(
        &debugcommands_string,
        "RECIPE_BUILDER_TAG_LOOKUP",
        &target_content,
    )?;

    println!("CopyClip completed successfully!");
    println!(
        "Extracted content length: {} bytes",
        result.extracted_content.len()
    );

    output_copyclip_result_with_update(
        &ctx.client,
        page_title,
        &target_content,
        &result.updated_content,
        output,
    )
    .await
}

async fn maintain_tech(ctx: &mut DstContext, output: Option<PathBuf>) -> Result<()> {
    let constants_string = ctx.read_zip_file("scripts/constants.lua")?;

    println!("Fetching wiki page content...");
    let page_title = "模块:Constants/Tech";
    let page = ctx.client.get_page(page_title).await?;

    let target_content = page
        .content
        .ok_or_else(|| Error::WikiApi("Wiki page has no content".to_string()))?;

    println!("Extracting TECH from constants.lua...");
    let result = process_copyclip(&constants_string, "TECH", &target_content)?;

    println!("CopyClip completed successfully!");
    println!(
        "Extracted content length: {} bytes",
        result.extracted_content.len()
    );

    output_copyclip_result_with_update(
        &ctx.client,
        page_title,
        &target_content,
        &result.updated_content,
        output,
    )
    .await
}

async fn maintain_crafting_filters(ctx: &mut DstContext, output: Option<PathBuf>) -> Result<()> {
    let filter_string = ctx.read_zip_file("scripts/recipes_filter.lua")?;

    println!("Fetching wiki page content...");
    let page_title = "模块:Constants/CraftingFilters";
    let page = ctx.client.get_page(page_title).await?;

    let target_content = page
        .content
        .ok_or_else(|| Error::WikiApi("Wiki page has no content".to_string()))?;

    println!(
        "Extracting CRAFTING_FILTERS.CHARACTER.recipes to CRAFTING_FILTERS.DECOR.recipes from recipes_filter.lua..."
    );
    let field_location = extract_field_assignment_range(
        &filter_string,
        "CRAFTING_FILTERS.CHARACTER.recipes",
        "CRAFTING_FILTERS.DECOR.recipes",
    )?;

    println!(
        "Extracted content length: {} bytes",
        field_location.content.len()
    );

    let marker_range = CopyClipProcessor::find_marker_range(&target_content)?;
    let updated_content = CopyClipProcessor::replace_between_markers(
        &target_content,
        &marker_range,
        &field_location.content,
    );

    println!("CopyClip completed successfully!");

    output_copyclip_result_with_update(
        &ctx.client,
        page_title,
        &target_content,
        &updated_content,
        output,
    )
    .await
}

async fn maintain_crafting_names(ctx: &mut DstContext, output: Option<PathBuf>) -> Result<()> {
    let po_file = ctx.parse_po_file("scripts/languages/chinese_s.po")?;

    let station_prefix = "STRINGS.UI.CRAFTING_STATION_FILTERS.";
    let filter_prefix = "STRINGS.UI.CRAFTING_FILTERS.";

    let mut crafting_stations: std::collections::BTreeMap<String, serde_json::Value> =
        std::collections::BTreeMap::new();
    let mut craftings: std::collections::BTreeMap<String, serde_json::Value> =
        std::collections::BTreeMap::new();

    for entry in &po_file.entries {
        if let Some(ref entry_ctx) = entry.msgctxt {
            if entry_ctx.starts_with(station_prefix) {
                let key = entry_ctx.strip_prefix(station_prefix).unwrap().to_string();
                crafting_stations.insert(
                    key,
                    serde_json::json!({
                        "station_en": entry.msgid.clone(),
                        "station_cn": entry.msgstr.clone(),
                    }),
                );
            } else if entry_ctx.starts_with(filter_prefix) {
                let key = entry_ctx.strip_prefix(filter_prefix).unwrap().to_string();
                craftings.insert(
                    key,
                    serde_json::json!({
                        "station_en": entry.msgid.clone(),
                        "station_cn": entry.msgstr.clone(),
                    }),
                );
            }
        }
    }

    let crafting_names = serde_json::json!({
        "crafting_stations": crafting_stations,
        "craftings": craftings
    });

    let json_content = serde_json::to_string_pretty(&crafting_names)?;
    println!(
        "Found {} crafting stations and {} craftings",
        crafting_names["crafting_stations"]
            .as_object()
            .map(|o| o.len())
            .unwrap_or(0),
        crafting_names["craftings"]
            .as_object()
            .map(|o| o.len())
            .unwrap_or(0)
    );

    println!("Fetching wiki page content...");
    let page_title = "模块:Constants/CraftingNames";
    let page = ctx.client.get_page(page_title).await?;

    let target_content = page
        .content
        .ok_or_else(|| Error::WikiApi("Wiki page has no content".to_string()))?;

    println!("Finding [[ and ]] markers...");
    let start_marker = "[[";
    let end_marker = "]]";

    let start_pos = target_content
        .find(start_marker)
        .ok_or_else(|| Error::ParseError("'[[' marker not found".to_string()))?;
    let end_pos = target_content
        .rfind(end_marker)
        .ok_or_else(|| Error::ParseError("']]' marker not found".to_string()))?;

    if start_pos >= end_pos {
        return Err(Error::ParseError("'[[' must appear before ']]'".to_string()));
    }

    let updated_content = format!(
        "{}{}\n{}",
        &target_content[..start_pos + start_marker.len()],
        &json_content,
        &target_content[end_pos..]
    );

    println!("CopyClip completed successfully!");

    output_copyclip_result_with_update(
        &ctx.client,
        page_title,
        &target_content,
        &updated_content,
        output,
    )
    .await
}

async fn output_json_result_with_update(
    client: &WikiClient,
    page_title: &str,
    wiki_data: &dst_huiji_wiki::mapping::WikiJsonData,
    output: Option<PathBuf>,
) -> Result<()> {
    let new_json = WikiDataConverter::to_json_string(wiki_data)?;

    if let Some(output_path) = output {
        std::fs::write(&output_path, &new_json)?;
        println!("Written {} records to {:?}", wiki_data.data.len(), output_path);
    }

    let historical_json = match client.get_json_data(page_title).await {
        Ok(json) => serde_json::to_string_pretty(&json)?,
        Err(e) => {
            println!("Warning: Failed to fetch current wiki data: {}", e);
            println!("Cannot compare with wiki data.");
            return Ok(());
        }
    };

    if new_json.trim() == historical_json.trim() {
        println!("No changes detected.");
        return Ok(());
    }

    println!("\n--- Changes Detected ---");
    println!("{}", diff_lines(&historical_json, &new_json));

    if prompt_confirm("Update wiki page?")? {
        println!("Updating wiki page: {}", page_title);

        let edit_result = client
            .edit_page(page_title, &new_json, Some("Update via dst-huiji-wiki tool"), false)
            .await?;

        println!(
            "Successfully updated page '{}' (new revision: {:?})",
            edit_result.title.as_deref().unwrap_or(page_title),
            edit_result.newrevid
        );
    } else {
        println!("Skipped updating wiki page.");
    }

    Ok(())
}

fn prompt_confirm(prompt: &str) -> Result<bool> {
    print!("{} (y/N): ", prompt);
    io::stdout().flush()?;

    let stdin = io::stdin();
    let mut line = String::new();
    stdin.lock().read_line(&mut line)?;

    let answer = line.trim().to_lowercase();
    Ok(answer == "y" || answer == "yes")
}

async fn output_copyclip_result_with_update(
    client: &WikiClient,
    page_title: &str,
    target_content: &str,
    updated_content: &str,
    output: Option<PathBuf>,
) -> Result<()> {
    if target_content == updated_content {
        println!("No changes detected.");
        return Ok(());
    }

    println!("\n--- Changes Detected ---");
    println!("{}", diff_lines(target_content, updated_content));

    if let Some(output_path) = output {
        std::fs::write(&output_path, updated_content)?;
        println!("Written updated content to {:?}", output_path);
    }

    if prompt_confirm("Update wiki page?")? {
        println!("Updating wiki page: {}", page_title);
        
        let edit_result = client
            .edit_page(page_title, updated_content, Some("Update via dst-huiji-wiki tool"), false)
            .await?;

        println!(
            "Successfully updated page '{}' (new revision: {:?})",
            edit_result.title.as_deref().unwrap_or(page_title),
            edit_result.newrevid
        );
    } else {
        println!("Skipped updating wiki page.");
    }

    Ok(())
}
