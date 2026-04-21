use clap::Parser;
use dst_huiji_wiki::mapping::{compare_and_report, WikiDataConverter};
use dst_huiji_wiki::models::PoEntry;
use dst_huiji_wiki::parser::{PoParser, RecipeParser};
use std::path::PathBuf;

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
    },
}

fn main() {
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

                    let wiki_data = if merge {
                        if let Some(compare_path) = &compare {
                            let historical_json =
                                std::fs::read_to_string(compare_path).expect("Failed to read compare file");
                            let historical_data = WikiDataConverter::parse_wiki_json(&historical_json)
                                .expect("Failed to parse historical data");
                            WikiDataConverter::convert_with_history(
                                &names_entries,
                                "Extract data from patch 722900",
                                &historical_data,
                            )
                        } else {
                            eprintln!("Error: --merge requires --compare");
                            std::process::exit(1);
                        }
                    } else {
                        WikiDataConverter::convert_to_wiki_json(
                            &names_entries,
                            "Extract data from patch 722900",
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
        } => {
            let lua_content = std::fs::read_to_string(&input).expect("Failed to read input file");
            let mut parser = RecipeParser::new();
            match parser.parse(&lua_content, input.to_str()) {
                Ok(recipes) => {
                    println!("Found {} recipes", recipes.len());

                    let wiki_data = if merge {
                        if let Some(compare_path) = &compare {
                            let historical_json =
                                std::fs::read_to_string(compare_path).expect("Failed to read compare file");
                            let historical_data = WikiDataConverter::parse_wiki_json(&historical_json)
                                .expect("Failed to parse historical data");
                            WikiDataConverter::convert_with_history(
                                &recipes,
                                "Extract data from patch 722900",
                                &historical_data,
                            )
                        } else {
                            eprintln!("Error: --merge requires --compare");
                            std::process::exit(1);
                        }
                    } else {
                        WikiDataConverter::convert_to_wiki_json(
                            &recipes,
                            "Extract data from patch 722900",
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
    }
}