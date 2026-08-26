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
        /// 跳过确认，直接写入维基
        #[arg(long)]
        yes: bool,
        /// 只生成产物与 diff，不写入维基（与 --yes 互斥）
        #[arg(long, conflicts_with = "yes")]
        dry_run: bool,
        /// 将机器可读的执行报告（JSON）写入该文件
        #[arg(long)]
        report_json: Option<PathBuf>,
    },
    MaintainDSTRecipes {
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// 跳过确认，直接写入维基
        #[arg(long)]
        yes: bool,
        /// 只生成产物与 diff，不写入维基（与 --yes 互斥）
        #[arg(long, conflicts_with = "yes")]
        dry_run: bool,
        /// 将机器可读的执行报告（JSON）写入该文件
        #[arg(long)]
        report_json: Option<PathBuf>,
    },
    MaintainCopyClip {
        #[arg(short = 't', long)]
        r#type: Option<String>,
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// 跳过确认，直接写入维基
        #[arg(long)]
        yes: bool,
        /// 只生成产物与 diff，不写入维基（与 --yes 互斥）
        #[arg(long, conflicts_with = "yes")]
        dry_run: bool,
        /// 将机器可读的执行报告（JSON）写入该文件
        #[arg(long)]
        report_json: Option<PathBuf>,
    },
    PrefabOverrides {
        #[arg(short, long)]
        input: PathBuf,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// 抓取维基主命名空间全量语料到本地目录（默认 wikis/，不入仓库）
    CorpusFetch {
        /// 忽略增量对账，全量重抓所有页面
        #[arg(long)]
        full: bool,
        /// 语料根目录（默认 wikis）
        #[arg(long)]
        dir: Option<PathBuf>,
        /// 只枚举与对账出报告，不写任何本地文件
        #[arg(long)]
        dry_run: bool,
    },
    /// 构建代码关联索引（基础设施A）并缓存到 output/atlas/<build>/
    UpdateIndex {
        /// 游戏脚本根目录（当前树或快照目录）
        root: PathBuf,
        /// 输出目录（默认 output/atlas/<build 号>/）
        #[arg(short, long)]
        out: Option<PathBuf>,
    },
    /// 启动 WebUI 服务器
    Serve {
        /// 监听地址（默认 127.0.0.1）
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        /// 监听端口
        #[arg(long, default_value_t = 8420)]
        port: u16,
    },
}

impl Commands {
    /// Stable machine-readable name used in logs and reports.
    pub fn name(&self) -> &'static str {
        match self {
            Commands::ParsePo { .. } => "parse-po",
            Commands::MapNames { .. } => "map-names",
            Commands::MapRecipes { .. } => "map-recipes",
            Commands::MaintainItemTable { .. } => "maintain-item-table",
            Commands::MaintainDSTRecipes { .. } => "maintain-dst-recipes",
            Commands::MaintainCopyClip { .. } => "maintain-copy-clip",
            Commands::PrefabOverrides { .. } => "prefab-overrides",
            Commands::CorpusFetch { .. } => "corpus-fetch",
            Commands::UpdateIndex { .. } => "update-index",
            Commands::Serve { .. } => "serve",
        }
    }
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
            Commands::MaintainItemTable {
                output,
                yes,
                dry_run,
                report_json,
            } => {
                assert_eq!(output, Some(PathBuf::from("item_table.json")));
                assert!(!yes);
                assert!(!dry_run);
                assert!(report_json.is_none());
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
            Commands::MaintainDSTRecipes { output, .. } => {
                assert!(output.is_none());
            }
            _ => panic!("Expected MaintainDSTRecipes command"),
        }
    }

    #[test]
    fn test_maintain_yes_flag() {
        let args = Args::try_parse_from([
            "dst-huiji-wiki",
            "maintain-item-table",
            "--yes",
            "--report-json",
            "report.json",
        ]);
        let args = args.unwrap();
        match args.command {
            Commands::MaintainItemTable {
                yes,
                dry_run,
                report_json,
                ..
            } => {
                assert!(yes);
                assert!(!dry_run);
                assert_eq!(report_json, Some(PathBuf::from("report.json")));
            }
            _ => panic!("Expected MaintainItemTable command"),
        }
    }

    #[test]
    fn test_update_index_command() {
        let args = Args::try_parse_from([
            "dst-huiji-wiki",
            "update-index",
            "/path/to/scripts",
            "--out",
            "output/atlas/custom",
        ]);
        assert!(args.is_ok());
        let args = args.unwrap();
        match args.command {
            Commands::UpdateIndex { root, out } => {
                assert_eq!(root, PathBuf::from("/path/to/scripts"));
                assert_eq!(out, Some(PathBuf::from("output/atlas/custom")));
            }
            _ => panic!("Expected UpdateIndex command"),
        }
    }

    #[test]
    fn test_update_index_command_defaults() {
        let args = Args::try_parse_from(["dst-huiji-wiki", "update-index", "scripts"]);
        assert!(args.is_ok());
        let args = args.unwrap();
        match args.command {
            Commands::UpdateIndex { root, out } => {
                assert_eq!(root, PathBuf::from("scripts"));
                assert!(out.is_none());
            }
            _ => panic!("Expected UpdateIndex command"),
        }
    }

    #[test]
    fn test_maintain_dry_run_flag() {
        let args = Args::try_parse_from(["dst-huiji-wiki", "maintain-dst-recipes", "--dry-run"]);
        let args = args.unwrap();
        match args.command {
            Commands::MaintainDSTRecipes { dry_run, .. } => assert!(dry_run),
            _ => panic!("Expected MaintainDSTRecipes command"),
        }
    }

    #[test]
    fn test_maintain_yes_and_dry_run_conflict() {
        let args =
            Args::try_parse_from(["dst-huiji-wiki", "maintain-copy-clip", "--yes", "--dry-run"]);
        assert!(
            args.is_err(),
            "--yes and --dry-run must be mutually exclusive"
        );
    }

    #[test]
    fn test_write_mode_flags_rejected_together_for_all_commands() {
        for cmd in [
            vec!["maintain-item-table"],
            vec!["maintain-dst-recipes"],
            vec!["maintain-copy-clip"],
        ] {
            let mut argv = vec!["dst-huiji-wiki"];
            argv.extend_from_slice(&cmd);
            argv.push("--yes");
            argv.push("--dry-run");
            assert!(
                Args::try_parse_from(&argv).is_err(),
                "expected conflict for {:?}",
                cmd
            );
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
