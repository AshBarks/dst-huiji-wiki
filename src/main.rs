use clap::Parser;
use dst_huiji_wiki::copyclip::process_copyclip;
use dst_huiji_wiki::mapping::{compare_and_report, WikiDataConverter, WikiMapper};
use dst_huiji_wiki::models::PoEntry;
use dst_huiji_wiki::parser::{PoParser, RecipeParser};
use dst_huiji_wiki::tech_report::TechReport;
use dst_huiji_wiki::wiki::WikiClient;
use std::path::PathBuf;

fn diff_lines(old: &str, new: &str) -> String {
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();
    
    let mut result = String::new();
    
    let max_lines = old_lines.len().max(new_lines.len());
    let mut changed = 0;
    let mut removed = 0;
    let mut unchanged = 0;
    
    for i in 0..max_lines {
        let old_line = old_lines.get(i);
        let new_line = new_lines.get(i);
        
        match (old_line, new_line) {
            (Some(o), Some(n)) if o.trim() == n.trim() => {
                unchanged += 1;
            }
            (Some(o), Some(n)) => {
                result.push_str(&format!("- {}\n", o));
                result.push_str(&format!("+ {}\n", n));
                changed += 1;
            }
            (Some(o), None) => {
                result.push_str(&format!("- {}\n", o));
                removed += 1;
            }
            (None, Some(n)) => {
                result.push_str(&format!("+ {}\n", n));
                changed += 1;
            }
            (None, None) => {}
        }
    }
    
    format!(
        "Summary: {} lines unchanged, {} lines changed, {} lines removed\n\n{}\n",
        unchanged, changed, removed, result
    )
}

#[derive(Parser, Debug)]
#[command(name = "dst-huiji-wiki")]
#[command(about = "饥荒联机版维基维护工具", long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Parser, Debug)]
enum Commands {
    ParsePo {
        #[arg(short, long)]
        input: PathBuf,
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[arg(short, long)]
        category: Option<String>,
    },
    MapNames {
        #[arg(short, long)]
        input: PathBuf,
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[arg(short, long)]
        compare: Option<PathBuf>,
        #[arg(short, long)]
        merge: bool,
        #[arg(short, long)]
        version: Option<String>,
    },
    MapRecipes {
        #[arg(short, long)]
        input: PathBuf,
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[arg(short, long)]
        compare: Option<PathBuf>,
        #[arg(short, long)]
        merge: bool,
        #[arg(long)]
        po_file: Option<PathBuf>,
        #[arg(short, long)]
        version: Option<String>,
    },
    MaintainItemTable {
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    MaintainDSTRecipes {
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    MaintainRecipeBuilderTagLookup {
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    MaintainTech {
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    MaintainCraftingFilters {
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    MaintainCraftingNames {
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

fn main() {
    tracing_subscriber::fmt::init();
    dotenvy::dotenv().ok();
    let args = Args::parse();

    match args.command {
        Commands::ParsePo {
            input,
            output,
            category,
        } => match PoParser::parse_from_file(input.to_str().unwrap()) {
            Ok(po_file) => {
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
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        },
        Commands::MapNames {
            input,
            output,
            compare,
            merge,
            version,
        } => {
            match PoParser::parse_from_file(input.to_str().unwrap()) {
                Ok(po_file) => {
                    let names_entries: Vec<PoEntry> = po_file
                        .entries
                        .iter()
                        .filter(|e| {
                            e.msgctxt
                                .as_ref()
                                .map(|ctx| ctx.starts_with("STRINGS.NAMES."))
                                .unwrap_or(false)
                        })
                        .cloned()
                        .collect();

                    println!("Found {} NAMES entries", names_entries.len());

                    let converter = WikiDataConverter::new();
                    let version_str = version.as_deref().unwrap_or("unknown");
                    let sources = format!("Extract data from DST version {}", version_str);

                    let wiki_data = if merge {
                        if let Some(compare_path) = &compare {
                            let historical_json =
                                std::fs::read_to_string(compare_path).expect("Failed to read compare file");
                            let historical_data = WikiDataConverter::parse_wiki_json(&historical_json)
                                .expect("Failed to parse historical data");
                            converter.convert_with_history(
                                &names_entries,
                                &sources,
                                &historical_data,
                            )
                        } else {
                            eprintln!("Error: --merge requires --compare");
                            std::process::exit(1);
                        }
                    } else {
                        converter.convert_to_wiki_json(
                            &names_entries,
                            &sources,
                        )
                    };

                    if let Some(compare_path) = compare {
                        if !merge {
                            let historical_json =
                                std::fs::read_to_string(&compare_path).expect("Failed to read compare file");
                            let historical_data = WikiDataConverter::parse_wiki_json(&historical_json)
                                .expect("Failed to parse historical data");

                            println!("\n{}", compare_and_report(&wiki_data, &historical_data));
                        }
                    }

                    if let Some(output_path) = output {
                        let json = WikiDataConverter::to_json_string(&wiki_data).unwrap();
                        std::fs::write(&output_path, json).unwrap();
                        println!("Written {} records to {:?}", wiki_data.data.len(), output_path);
                    } else {
                        println!("\nFirst 5 records:");
                        for (i, record) in wiki_data.data.iter().take(5).enumerate() {
                            println!("{}: {:?}", i + 1, record);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Commands::MapRecipes {
            input,
            output,
            compare,
            merge,
            po_file,
            version,
        } => {
            let lua_content = std::fs::read_to_string(&input).expect("Failed to read input file");
            let mut parser = RecipeParser::new();
            match parser.parse(&lua_content, input.to_str()) {
                Ok(recipes) => {
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
                        if let Some(compare_path) = &compare {
                            let historical_json =
                                std::fs::read_to_string(compare_path).expect("Failed to read compare file");
                            let historical_data = WikiDataConverter::parse_wiki_json(&historical_json)
                                .expect("Failed to parse historical data");
                            let mut data = converter.convert_recipes(
                                &recipes,
                                &sources,
                            );
                            dst_huiji_wiki::models::Recipe::merge_with_history(&mut data, &historical_data);
                            data
                        } else {
                            eprintln!("Error: --merge requires --compare");
                            std::process::exit(1);
                        }
                    } else {
                        converter.convert_recipes(
                            &recipes,
                            &sources,
                        )
                    };

                    if let Some(compare_path) = compare {
                        if !merge {
                            let historical_json =
                                std::fs::read_to_string(&compare_path).expect("Failed to read compare file");
                            let historical_data = WikiDataConverter::parse_wiki_json(&historical_json)
                                .expect("Failed to parse historical data");

                            println!("\n{}", compare_and_report(&wiki_data, &historical_data));
                        }
                    }

                    if let Some(output_path) = output {
                        let json = WikiDataConverter::to_json_string(&wiki_data).unwrap();
                        std::fs::write(&output_path, json).unwrap();
                        println!("Written {} records to {:?}", wiki_data.data.len(), output_path);
                    } else {
                        println!("\nFirst 5 records:");
                        for (i, record) in wiki_data.data.iter().take(5).enumerate() {
                            println!("{}: {:?}", i + 1, record);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Commands::MaintainItemTable { output } => {
            match tokio::runtime::Runtime::new()
                .expect("Failed to create tokio runtime")
                .block_on(maintain_item_table(output))
            {
                Ok(()) => {}
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Commands::MaintainDSTRecipes { output } => {
            match tokio::runtime::Runtime::new()
                .expect("Failed to create tokio runtime")
                .block_on(maintain_dst_recipes(output))
            {
                Ok(()) => {}
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Commands::MaintainRecipeBuilderTagLookup { output } => {
            match tokio::runtime::Runtime::new()
                .expect("Failed to create tokio runtime")
                .block_on(maintain_recipe_builder_tag_lookup(output))
            {
                Ok(()) => {}
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Commands::MaintainTech { output } => {
            match tokio::runtime::Runtime::new()
                .expect("Failed to create tokio runtime")
                .block_on(maintain_tech(output))
            {
                Ok(()) => {}
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Commands::MaintainCraftingFilters { output } => {
            match tokio::runtime::Runtime::new()
                .expect("Failed to create tokio runtime")
                .block_on(maintain_crafting_filters(output))
            {
                Ok(()) => {}
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Commands::MaintainCraftingNames { output } => {
            match tokio::runtime::Runtime::new()
                .expect("Failed to create tokio runtime")
                .block_on(maintain_crafting_names(output))
            {
                Ok(()) => {}
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }
}

async fn maintain_item_table(output: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    let dst_root = std::env::var("DST__ROOT")
        .map_err(|_| "DST__ROOT environment variable not set")?;
    
    let dst_path = std::path::Path::new(&dst_root);
    if !dst_path.exists() {
        return Err(format!("DST directory does not exist: {}", dst_root).into());
    }
    
    let version_file = dst_path.join("version.txt");
    let version = std::fs::read_to_string(&version_file)
        .map(|v| v.trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    println!("DST version: {}", version);
    
    let scripts_zip = dst_path.join("data/databundles/scripts.zip");
    if !scripts_zip.exists() {
        return Err(format!("scripts.zip not found: {:?}", scripts_zip).into());
    }
    
    println!("Reading scripts.zip from {:?}", scripts_zip);
    
    let file = std::fs::File::open(&scripts_zip)?;
    let reader = std::io::BufReader::new(file);
    let mut archive = zip::ZipArchive::new(reader)?;
    
    let po_content = archive.by_name("scripts/languages/chinese_s.po")
        .map_err(|_| "scripts/languages/chinese_s.po not found in scripts.zip")?;
    
    let mut po_content = std::io::BufReader::new(po_content);
    let mut po_string = String::new();
    std::io::Read::read_to_string(&mut po_content, &mut po_string)?;
    
    println!("Parsing chinese_s.po...");
    let po_file = PoParser::parse(&po_string)?;
    let names_entries: Vec<PoEntry> = po_file
        .entries
        .iter()
        .filter(|e| {
            e.msgctxt
                .as_ref()
                .map(|ctx| ctx.starts_with("STRINGS.NAMES."))
                .unwrap_or(false)
        })
        .cloned()
        .collect();
    
    println!("Found {} NAMES entries", names_entries.len());
    
    let converter = WikiDataConverter::new();
    
    println!("Fetching historical data from wiki...");
    let client = WikiClient::from_env()
        .map_err(|e| format!("Failed to create wiki client: {}", e))?;
    
    let historical_data = match client.get_json_data("Data:ItemTable.tabx").await {
        Ok(historical_json) => {
            Some(WikiDataConverter::parse_wiki_json(&historical_json.to_string())?)
        }
        Err(e) => {
            println!("Warning: Failed to fetch historical data from wiki: {}", e);
            println!("Proceeding without historical data...");
            None
        }
    };
    
    let sources = format!("Extract data from DST version {}", version);
    let mut wiki_data = converter.convert_to_wiki_json(
        &names_entries,
        &sources,
    );
    
    if let Some(ref historical) = historical_data {
        PoEntry::merge_with_history(&mut wiki_data, historical);
        println!("\n{}", compare_and_report(&wiki_data, historical));
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

async fn maintain_recipe_builder_tag_lookup(
    output: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    let dst_root = std::env::var("DST__ROOT")
        .map_err(|_| "DST__ROOT environment variable not set")?;

    let dst_path = std::path::Path::new(&dst_root);
    if !dst_path.exists() {
        return Err(format!("DST directory does not exist: {}", dst_root).into());
    }

    let version_file = dst_path.join("version.txt");
    let version = std::fs::read_to_string(&version_file)
        .map(|v| v.trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    println!("DST version: {}", version);

    let scripts_zip = dst_path.join("data/databundles/scripts.zip");
    if !scripts_zip.exists() {
        return Err(format!("scripts.zip not found: {:?}", scripts_zip).into());
    }

    println!("Reading scripts.zip from {:?}", scripts_zip);

    let file = std::fs::File::open(&scripts_zip)?;
    let reader = std::io::BufReader::new(file);
    let mut archive = zip::ZipArchive::new(reader)?;

    println!("Reading debugcommands.lua from scripts.zip...");
    let debugcommands_content = archive.by_name("scripts/debugcommands.lua")
        .map_err(|_| "scripts/debugcommands.lua not found in scripts.zip")?;

    let mut debugcommands_content = std::io::BufReader::new(debugcommands_content);
    let mut debugcommands_string = String::new();
    std::io::Read::read_to_string(&mut debugcommands_content, &mut debugcommands_string)?;

    println!("Fetching wiki page content...");
    let client = WikiClient::from_env()
        .map_err(|e| format!("Failed to create wiki client: {}", e))?;

    let page = client
        .get_page("模块:Constants/RecipeBuilderTagLookup")
        .await
        .map_err(|e| format!("Failed to get wiki page: {}", e))?;

    let target_content = page.content
        .ok_or("Wiki page has no content")?;

    println!("Extracting RECIPE_BUILDER_TAG_LOOKUP from debugcommands.lua...");
    let result = process_copyclip(
        &debugcommands_string,
        "RECIPE_BUILDER_TAG_LOOKUP",
        &target_content,
    )?;

    println!("CopyClip completed successfully!");
    println!("Extracted content length: {} bytes", result.extracted_content.len());

    if target_content == result.updated_content {
        println!("No changes detected.");
    } else {
        println!("\n--- Changes Detected ---");
        println!("{}", diff_lines(&target_content, &result.updated_content));
    }

    if let Some(output_path) = output {
        std::fs::write(&output_path, &result.updated_content)?;
        println!("Written updated content to {:?}", output_path);
    }

    Ok(())
}

async fn maintain_tech(output: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    let dst_root = std::env::var("DST__ROOT")
        .map_err(|_| "DST__ROOT environment variable not set")?;

    let dst_path = std::path::Path::new(&dst_root);
    if !dst_path.exists() {
        return Err(format!("DST directory does not exist: {}", dst_root).into());
    }

    let version_file = dst_path.join("version.txt");
    let version = std::fs::read_to_string(&version_file)
        .map(|v| v.trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    println!("DST version: {}", version);

    let scripts_zip = dst_path.join("data/databundles/scripts.zip");
    if !scripts_zip.exists() {
        return Err(format!("scripts.zip not found: {:?}", scripts_zip).into());
    }

    println!("Reading scripts.zip from {:?}", scripts_zip);

    let file = std::fs::File::open(&scripts_zip)?;
    let reader = std::io::BufReader::new(file);
    let mut archive = zip::ZipArchive::new(reader)?;

    println!("Reading constants.lua from scripts.zip...");
    let constants_content = archive.by_name("scripts/constants.lua")
        .map_err(|_| "scripts/constants.lua not found in scripts.zip")?;

    let mut constants_content = std::io::BufReader::new(constants_content);
    let mut constants_string = String::new();
    std::io::Read::read_to_string(&mut constants_content, &mut constants_string)?;

    println!("Fetching wiki page content...");
    let client = WikiClient::from_env()
        .map_err(|e| format!("Failed to create wiki client: {}", e))?;

    let page = client
        .get_page("模块:Constants/Tech")
        .await
        .map_err(|e| format!("Failed to get wiki page: {}", e))?;

    let target_content = page.content
        .ok_or("Wiki page has no content")?;

    println!("Extracting TECH from constants.lua...");
    let result = process_copyclip(
        &constants_string,
        "TECH",
        &target_content,
    )?;

    println!("CopyClip completed successfully!");
    println!("Extracted content length: {} bytes", result.extracted_content.len());

    if target_content == result.updated_content {
        println!("No changes detected.");
    } else {
        println!("\n--- Changes Detected ---");
        println!("{}", diff_lines(&target_content, &result.updated_content));
    }

    if let Some(output_path) = output {
        std::fs::write(&output_path, &result.updated_content)?;
        println!("Written updated content to {:?}", output_path);
    }

    Ok(())
}

async fn maintain_crafting_filters(
    output: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    let dst_root = std::env::var("DST__ROOT")
        .map_err(|_| "DST__ROOT environment variable not set")?;

    let dst_path = std::path::Path::new(&dst_root);
    if !dst_path.exists() {
        return Err(format!("DST directory does not exist: {}", dst_root).into());
    }

    let version_file = dst_path.join("version.txt");
    let version = std::fs::read_to_string(&version_file)
        .map(|v| v.trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    println!("DST version: {}", version);

    let scripts_zip = dst_path.join("data/databundles/scripts.zip");
    if !scripts_zip.exists() {
        return Err(format!("scripts.zip not found: {:?}", scripts_zip).into());
    }

    println!("Reading scripts.zip from {:?}", scripts_zip);

    let file = std::fs::File::open(&scripts_zip)?;
    let reader = std::io::BufReader::new(file);
    let mut archive = zip::ZipArchive::new(reader)?;

    println!("Reading recipes_filter.lua from scripts.zip...");
    let filter_content = archive.by_name("scripts/recipes_filter.lua")
        .map_err(|_| "scripts/recipes_filter.lua not found in scripts.zip")?;

    let mut filter_content = std::io::BufReader::new(filter_content);
    let mut filter_string = String::new();
    std::io::Read::read_to_string(&mut filter_content, &mut filter_string)?;

    println!("Fetching wiki page content...");
    let client = WikiClient::from_env()
        .map_err(|e| format!("Failed to create wiki client: {}", e))?;

    let page = client
        .get_page("模块:Constants/CraftingFilters")
        .await
        .map_err(|e| format!("Failed to get wiki page: {}", e))?;

    let target_content = page.content
        .ok_or("Wiki page has no content")?;

    println!("Extracting CRAFTING_FILTERS.CHARACTER.recipes to CRAFTING_FILTERS.DECOR.recipes from recipes_filter.lua...");
    let field_location = dst_huiji_wiki::parser::extract_field_assignment_range(
        &filter_string,
        "CRAFTING_FILTERS.CHARACTER.recipes",
        "CRAFTING_FILTERS.DECOR.recipes",
    )?;

    println!("Extracted content length: {} bytes", field_location.content.len());

    let marker_range = dst_huiji_wiki::copyclip::CopyClipProcessor::find_marker_range(&target_content)?;
    let updated_content = dst_huiji_wiki::copyclip::CopyClipProcessor::replace_between_markers(
        &target_content,
        &marker_range,
        &field_location.content,
    );

    println!("CopyClip completed successfully!");

    if target_content == updated_content {
        println!("No changes detected.");
    } else {
        println!("\n--- Changes Detected ---");
        println!("{}", diff_lines(&target_content, &updated_content));
    }

    if let Some(output_path) = output {
        std::fs::write(&output_path, &updated_content)?;
        println!("Written updated content to {:?}", output_path);
    }

    Ok(())
}

async fn maintain_crafting_names(
    output: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    let dst_root = std::env::var("DST__ROOT")
        .map_err(|_| "DST__ROOT environment variable not set")?;

    let dst_path = std::path::Path::new(&dst_root);
    if !dst_path.exists() {
        return Err(format!("DST directory does not exist: {}", dst_root).into());
    }

    let version_file = dst_path.join("version.txt");
    let version = std::fs::read_to_string(&version_file)
        .map(|v| v.trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    println!("DST version: {}", version);

    let scripts_zip = dst_path.join("data/databundles/scripts.zip");
    if !scripts_zip.exists() {
        return Err(format!("scripts.zip not found: {:?}", scripts_zip).into());
    }

    println!("Reading scripts.zip from {:?}", scripts_zip);

    let file = std::fs::File::open(&scripts_zip)?;
    let reader = std::io::BufReader::new(file);
    let mut archive = zip::ZipArchive::new(reader)?;

    println!("Reading chinese_s.po from scripts.zip...");
    let po_content = archive.by_name("scripts/languages/chinese_s.po")
        .map_err(|_| "scripts/languages/chinese_s.po not found in scripts.zip")?;

    let mut po_content = std::io::BufReader::new(po_content);
    let mut po_string = String::new();
    std::io::Read::read_to_string(&mut po_content, &mut po_string)?;

    println!("Parsing chinese_s.po...");
    let po_file = PoParser::parse(&po_string)?;

    let station_prefix = "STRINGS.UI.CRAFTING_STATION_FILTERS.";
    let filter_prefix = "STRINGS.UI.CRAFTING_FILTERS.";

    let mut crafting_stations: std::collections::BTreeMap<String, serde_json::Value> = std::collections::BTreeMap::new();
    let mut craftings: std::collections::BTreeMap<String, serde_json::Value> = std::collections::BTreeMap::new();

    for entry in &po_file.entries {
        if let Some(ref ctx) = entry.msgctxt {
            if ctx.starts_with(station_prefix) {
                let key = ctx.strip_prefix(station_prefix).unwrap().to_string();
                crafting_stations.insert(key, serde_json::json!({
                    "station_en": entry.msgid.clone(),
                    "station_cn": entry.msgstr.clone(),
                }));
            } else if ctx.starts_with(filter_prefix) {
                let key = ctx.strip_prefix(filter_prefix).unwrap().to_string();
                craftings.insert(key, serde_json::json!({
                    "station_en": entry.msgid.clone(),
                    "station_cn": entry.msgstr.clone(),
                }));
            }
        }
    }

    let crafting_names = serde_json::json!({
        "crafting_stations": crafting_stations,
        "craftings": craftings
    });

    let json_content = serde_json::to_string_pretty(&crafting_names)?;
    println!("Found {} crafting stations and {} craftings", 
        crafting_names["crafting_stations"].as_object().map(|o| o.len()).unwrap_or(0),
        crafting_names["craftings"].as_object().map(|o| o.len()).unwrap_or(0)
    );

    println!("Fetching wiki page content...");
    let client = WikiClient::from_env()
        .map_err(|e| format!("Failed to create wiki client: {}", e))?;

    let page = client
        .get_page("模块:Constants/CraftingNames")
        .await
        .map_err(|e| format!("Failed to get wiki page: {}", e))?;

    let target_content = page.content
        .ok_or("Wiki page has no content")?;

    println!("Finding [[ and ]] markers...");
    let start_marker = "[[";
    let end_marker = "]]";

    let start_pos = target_content
        .find(start_marker)
        .ok_or("'[[' marker not found")?;
    let end_pos = target_content
        .rfind(end_marker)
        .ok_or("']]' marker not found")?;

    if start_pos >= end_pos {
        return Err("'[[' must appear before ']]'".into());
    }

    let updated_content = format!(
        "{}{}\n{}",
        &target_content[..start_pos + start_marker.len()],
        &json_content,
        &target_content[end_pos..]
    );

    println!("CopyClip completed successfully!");

    if target_content == updated_content {
        println!("No changes detected.");
    } else {
        println!("\n--- Changes Detected ---");
        println!("{}", diff_lines(&target_content, &updated_content));
    }

    if let Some(output_path) = output {
        std::fs::write(&output_path, &updated_content)?;
        println!("Written updated content to {:?}", output_path);
    }

    Ok(())
}

async fn maintain_dst_recipes(output: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    let dst_root = std::env::var("DST__ROOT")
        .map_err(|_| "DST__ROOT environment variable not set")?;
    
    let dst_path = std::path::Path::new(&dst_root);
    if !dst_path.exists() {
        return Err(format!("DST directory does not exist: {}", dst_root).into());
    }
    
    let version_file = dst_path.join("version.txt");
    let version = std::fs::read_to_string(&version_file)
        .map(|v| v.trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    println!("DST version: {}", version);
    
    let scripts_zip = dst_path.join("data/databundles/scripts.zip");
    if !scripts_zip.exists() {
        return Err(format!("scripts.zip not found: {:?}", scripts_zip).into());
    }
    
    println!("Reading scripts.zip from {:?}", scripts_zip);
    
    let file = std::fs::File::open(&scripts_zip)?;
    let reader = std::io::BufReader::new(file);
    let mut archive = zip::ZipArchive::new(reader)?;
    
    let recipes_content = archive.by_name("scripts/recipes.lua")
        .map_err(|_| "scripts/recipes.lua not found in scripts.zip")?;
    
    let mut recipes_content = std::io::BufReader::new(recipes_content);
    let mut recipes_string = String::new();
    std::io::Read::read_to_string(&mut recipes_content, &mut recipes_string)?;
    
    drop(recipes_content);
    
    println!("Parsing recipes.lua...");
    let mut parser = RecipeParser::new();
    let recipes = parser.parse(&recipes_string, Some("scripts/recipes.lua"))?;
    
    println!("Found {} recipes", recipes.len());
    
    println!("\nFetching Tech data from wiki for comparison...");
    let client = WikiClient::from_env()
        .map_err(|e| format!("Failed to create wiki client: {}", e))?;
    
    let mut tech_report = TechReport::from_recipes(&recipes);
    
    match client.get_page("模块:RenderRecsByIngre/Data").await {
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
    let po_content = archive.by_name("scripts/languages/chinese_s.po")
        .map_err(|_| "scripts/languages/chinese_s.po not found in scripts.zip")?;
    
    let mut po_content = std::io::BufReader::new(po_content);
    let mut po_string = String::new();
    std::io::Read::read_to_string(&mut po_content, &mut po_string)?;
    
    let po_file = PoParser::parse(&po_string)?;
    println!("Loaded {} PO entries for desc lookup", po_file.entries.len());
    
    let converter = WikiDataConverter::with_po_entries(po_file.entries.clone());
    
    println!("Fetching historical data from wiki...");
    
    let historical_data = match client.get_json_data("Data:DSTRecipes.tabx").await {
        Ok(historical_json) => {
            Some(WikiDataConverter::parse_wiki_json(&historical_json.to_string())?)
        }
        Err(e) => {
            println!("Warning: Failed to fetch historical data from wiki: {}", e);
            println!("Proceeding without historical data...");
            None
        }
    };
    
    let sources = format!("Extract data from DST version {}", version);
    let mut wiki_data = converter.convert_recipes(
        &recipes,
        &sources,
    );
    
    if let Some(ref historical) = historical_data {
        dst_huiji_wiki::models::Recipe::merge_with_history(&mut wiki_data, historical);
        println!("\n{}", compare_and_report(&wiki_data, historical));
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