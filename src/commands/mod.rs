mod maintain;

use clap::Parser;
use std::path::PathBuf;

pub use maintain::run;

#[derive(Parser, Debug)]
#[command(name = "dst-huiji-wiki")]
#[command(about = "饥荒联机版维基维护工具", long_about = None)]
pub struct Args {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Parser, Debug, PartialEq)]
pub enum Commands {
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
    MaintainCopyClip {
        #[arg(short = 't', long)]
        r#type: Option<String>,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_po_command() {
        let args = Args::try_parse_from(["dst-huiji-wiki", "parse-po", "-i", "test.po"]);
        assert!(args.is_ok());
        let args = args.unwrap();
        match args.command {
            Commands::ParsePo {
                input,
                output,
                category,
            } => {
                assert_eq!(input, PathBuf::from("test.po"));
                assert!(output.is_none());
                assert!(category.is_none());
            }
            _ => panic!("Expected ParsePo command"),
        }
    }

    #[test]
    fn test_parse_po_command_with_all_options() {
        let args = Args::try_parse_from([
            "dst-huiji-wiki",
            "parse-po",
            "-i",
            "test.po",
            "-o",
            "output.json",
            "-c",
            "NAMES",
        ]);
        assert!(args.is_ok());
        let args = args.unwrap();
        match args.command {
            Commands::ParsePo {
                input,
                output,
                category,
            } => {
                assert_eq!(input, PathBuf::from("test.po"));
                assert_eq!(output, Some(PathBuf::from("output.json")));
                assert_eq!(category, Some("NAMES".to_string()));
            }
            _ => panic!("Expected ParsePo command"),
        }
    }

    #[test]
    fn test_map_names_command() {
        let args = Args::try_parse_from([
            "dst-huiji-wiki",
            "map-names",
            "-i",
            "chinese_s.po",
            "-o",
            "names.json",
            "-v",
            "1.0.0",
        ]);
        assert!(args.is_ok());
        let args = args.unwrap();
        match args.command {
            Commands::MapNames {
                input,
                output,
                compare,
                merge,
                version,
            } => {
                assert_eq!(input, PathBuf::from("chinese_s.po"));
                assert_eq!(output, Some(PathBuf::from("names.json")));
                assert!(compare.is_none());
                assert!(!merge);
                assert_eq!(version, Some("1.0.0".to_string()));
            }
            _ => panic!("Expected MapNames command"),
        }
    }

    #[test]
    fn test_map_names_command_with_merge() {
        let args = Args::try_parse_from([
            "dst-huiji-wiki",
            "map-names",
            "-i",
            "chinese_s.po",
            "--compare",
            "old.json",
            "--merge",
        ]);
        assert!(args.is_ok());
        let args = args.unwrap();
        match args.command {
            Commands::MapNames {
                input,
                compare,
                merge,
                ..
            } => {
                assert_eq!(input, PathBuf::from("chinese_s.po"));
                assert_eq!(compare, Some(PathBuf::from("old.json")));
                assert!(merge);
            }
            _ => panic!("Expected MapNames command"),
        }
    }

    #[test]
    fn test_map_recipes_command() {
        let args = Args::try_parse_from([
            "dst-huiji-wiki",
            "map-recipes",
            "-i",
            "recipes.lua",
            "-o",
            "recipes.json",
            "--po-file",
            "chinese_s.po",
        ]);
        assert!(args.is_ok());
        let args = args.unwrap();
        match args.command {
            Commands::MapRecipes {
                input,
                output,
                po_file,
                ..
            } => {
                assert_eq!(input, PathBuf::from("recipes.lua"));
                assert_eq!(output, Some(PathBuf::from("recipes.json")));
                assert_eq!(po_file, Some(PathBuf::from("chinese_s.po")));
            }
            _ => panic!("Expected MapRecipes command"),
        }
    }

    #[test]
    fn test_maintain_item_table_command() {
        let args = Args::try_parse_from([
            "dst-huiji-wiki",
            "maintain-item-table",
            "-o",
            "item_table.json",
        ]);
        assert!(args.is_ok());
        let args = args.unwrap();
        match args.command {
            Commands::MaintainItemTable { output } => {
                assert_eq!(output, Some(PathBuf::from("item_table.json")));
            }
            _ => panic!("Expected MaintainItemTable command"),
        }
    }

    #[test]
    fn test_maintain_dst_recipes_command() {
        let args = Args::try_parse_from(["dst-huiji-wiki", "maintain-dst-recipes"]);
        assert!(args.is_ok());
        let args = args.unwrap();
        match args.command {
            Commands::MaintainDSTRecipes { output } => {
                assert!(output.is_none());
            }
            _ => panic!("Expected MaintainDSTRecipes command"),
        }
    }

    #[test]
    fn test_maintain_copyclip_command() {
        let args = Args::try_parse_from(["dst-huiji-wiki", "maintain-copy-clip", "-t", "tech"]);
        assert!(args.is_ok());
        let args = args.unwrap();
        match args.command {
            Commands::MaintainCopyClip { r#type, .. } => {
                assert_eq!(r#type, Some("tech".to_string()));
            }
            _ => panic!("Expected MaintainCopyClip command"),
        }
    }

    #[test]
    fn test_maintain_copyclip_command_all_types() {
        let types = vec![
            "recipe_builder_tag_lookup",
            "tech",
            "crafting_filters",
            "crafting_names",
        ];
        for t in types {
            let args = Args::try_parse_from(["dst-huiji-wiki", "maintain-copy-clip", "-t", t]);
            assert!(args.is_ok(), "Failed to parse type: {}", t);
        }
    }

    #[test]
    fn test_commands_equality() {
        let cmd1 = Commands::ParsePo {
            input: PathBuf::from("test.po"),
            output: None,
            category: None,
        };
        let cmd2 = Commands::ParsePo {
            input: PathBuf::from("test.po"),
            output: None,
            category: None,
        };
        let cmd3 = Commands::ParsePo {
            input: PathBuf::from("other.po"),
            output: None,
            category: None,
        };
        assert_eq!(cmd1, cmd2);
        assert_ne!(cmd1, cmd3);
    }

    #[test]
    fn test_args_debug() {
        let args = Args::try_parse_from(["dst-huiji-wiki", "parse-po", "-i", "test.po"]).unwrap();
        let debug_str = format!("{:?}", args);
        assert!(debug_str.contains("ParsePo"));
    }
}
