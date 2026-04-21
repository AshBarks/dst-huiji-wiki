use clap::Parser;
use dst_huiji_wiki::parser::PoParser;
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
}

fn main() {
    let args = Args::parse();

    match args.command {
        Commands::ParsePo { input, output, category } => {
            match PoParser::parse_from_file(input.to_str().unwrap()) {
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
            }
        }
    }
}