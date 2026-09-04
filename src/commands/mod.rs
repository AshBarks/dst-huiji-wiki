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
    /// 把游戏 skilltree_<char>.lua 提取为 模块:Skilltree/<Char> 子页面的
    /// defs JSON 并维护维基子页面（保留页内 metainfo/icon_url）
    #[command(name = "skilltree-wiki")]
    SkillTreeWiki {
        /// 只处理名字包含该子串的角色（如 walter）
        #[arg(short, long)]
        character: Option<String>,
        /// 同时把每个子页面内容写到该目录（<Char>.lua）
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// 使用指定 scripts 快照（默认当前 scripts 树）
        #[arg(long)]
        snapshot: Option<String>,
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
    /// 把技能树数据导出为本地 JSON 文件（纯本地，不访问维基）
    #[command(name = "skilltree-export")]
    SkillTreeExport {
        /// 只处理名字包含该子串的角色（如 walter）
        #[arg(short, long)]
        character: Option<String>,
        /// 输出目录（默认 output/skilltree），每个角色写入 <角色>.json
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// 使用指定 scripts 快照（默认当前 scripts 树）
        #[arg(long)]
        snapshot: Option<String>,
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
    /// 同步更新后的游戏 scripts:归档旧树为快照、解压新 scripts.zip(纯本地操作)
    ScriptsSync {
        /// 忽略版本一致,强制同步
        #[arg(long)]
        force: bool,
        /// 只报告将要执行的动作,不修改任何文件
        #[arg(long)]
        dry_run: bool,
        /// 版本状态文件路径(默认 ./dst_version.txt)
        #[arg(long)]
        state: Option<PathBuf>,
        /// 将机器可读的执行报告（JSON）写入该文件
        #[arg(long)]
        report_json: Option<PathBuf>,
    },
    /// 处理游戏图片资源:解压 images.zip、ktech 解码、按 xml 切割(纯本地操作)
    ImagesSync {
        /// 忽略增量与幂等检查,全量重跑
        #[arg(long)]
        force: bool,
        /// 只盘点并报告计划,不写任何文件
        #[arg(long)]
        dry_run: bool,
        /// 将机器可读的执行报告（JSON）写入该文件
        #[arg(long)]
        report_json: Option<PathBuf>,
    },
    /// 动画历史同步:更新后扫描 data/anim，归档 zip/dyn 到 ANIM__OUT_DIR
    AnimSync {
        /// 忽略幂等检查,强制重新扫描/归档
        #[arg(long)]
        force: bool,
        /// 只盘点并报告计划,不写任何文件
        #[arg(long)]
        dry_run: bool,
        /// 版本标签(默认 DST__ROOT/version.txt)
        #[arg(long)]
        label: Option<String>,
        /// 历史根目录(默认 $ANIM__OUT_DIR 或 output/anim)
        #[arg(long)]
        out: Option<PathBuf>,
        /// 将机器可读的执行报告（JSON）写入该文件
        #[arg(long)]
        report_json: Option<PathBuf>,
    },
    /// 对比两个动画目录的 zip/dyn 结构化 diff
    AnimDiff {
        /// 旧动画目录
        old: PathBuf,
        /// 新动画目录
        new: PathBuf,
        /// 只对比指定相对路径,如 dynamic/abigail_ice.dyn
        #[arg(long)]
        zip: Option<String>,
        /// 将机器可读的执行报告（JSON）写入该文件
        #[arg(long)]
        report_json: Option<PathBuf>,
    },
    /// 构建 prefab 与动画文件的关联索引（纯本地，不做皮肤关联）
    AnimIndex {
        /// 游戏脚本根目录（当前树或快照目录）
        scripts: PathBuf,
        /// 动画资源目录；缺省由 scripts 路径推导为 ../data/anim
        #[arg(long)]
        anim: Option<PathBuf>,
        /// 输出 JSON 文件路径（默认 output/anim-index.json）
        #[arg(short, long)]
        out: Option<PathBuf>,
        /// 将机器可读的执行报告（JSON）写入该文件
        #[arg(long)]
        report_json: Option<PathBuf>,
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
        /// recentchanges 增量通道(检查点缺失/过期自动回落枚举对账)
        #[arg(long)]
        rc: bool,
    },
    /// 快照差异 + 关联影响评估（M1，只读）：产出 impact.json 与 changes.patch
    UpdateScan {
        /// 旧快照（时间戳或目录名）
        old: String,
        /// 新快照（时间戳、目录名，或 "current" 表示当前 scripts 树）
        new: String,
        /// 输出目录（默认 output/scan/<old>_<new>/）
        #[arg(short, long)]
        out: Option<PathBuf>,
        /// 语料 host 根目录（wikis/<host>/），提供时附加 Layer B 定级摘要
        #[arg(long)]
        corpus: Option<PathBuf>,
        /// 输出 fn 标注骨架（prefabs 前 50 文件 + hound.lua）
        #[arg(long)]
        annotate: Option<PathBuf>,
    },
    /// 构建代码关联索引（基础设施A）并缓存到 output/atlas/<build>/
    UpdateIndex {
        /// 游戏脚本根目录（当前树或快照目录）
        root: PathBuf,
        /// 输出目录（默认 output/atlas/<build 号>/）
        #[arg(short, long)]
        out: Option<PathBuf>,
    },
    /// 从本地语料树重建派生索引（prefab 注册表等，纯本地操作）
    CorpusIndex {
        /// 语料根目录（默认 wikis，其下需恰好一个 host 树）
        #[arg(long)]
        dir: Option<PathBuf>,
        /// 代码侧 index.json 路径：额外产出 join_report.json 校准报告
        #[arg(long)]
        join: Option<PathBuf>,
        /// 只构建并报告统计，不写工件
        #[arg(long)]
        dry_run: bool,
    },
    /// Page→Symbol 标注 CLI：生成高引用 symbol 证据包和 Prompt，
    /// 可选读取已有 LLM 输出并生成跨页一致性报告（纯本地，不写 wiki）
    SymbolAnnotate {
        /// 游戏脚本根目录（当前树或快照目录）
        root: PathBuf,
        /// 语料 host 根目录（wikis/<host>/）
        #[arg(long)]
        corpus: PathBuf,
        /// 只处理引用量最高的前 N 个 symbol
        #[arg(long, default_value_t = 20)]
        limit: usize,
        /// 输出目录（默认 output/symbol-annotate/）
        #[arg(long)]
        out: Option<PathBuf>,
        /// 可选的 LLM/人工标注结果文件（JSON 数组或 SymbolAnnotationResponse）
        #[arg(long)]
        verdicts: Option<PathBuf>,
        /// 配置了 LLM__API_KEY 时直接调用大模型生成标注；未配置则跳过
        #[arg(long)]
        llm: bool,
        /// LLM 分批大小：每批最多交给模型的页面数（0 = 不按页数设限）
        #[arg(long, default_value_t = 40)]
        batch_pages: usize,
        /// 每批渲染输入的字节预算（0 = 不按字节设限，默认 32000）
        #[arg(long, default_value_t = dst_huiji_wiki::update::DEFAULT_BATCH_MAX_CHARS)]
        batch_max_chars: usize,
        /// 零候选证据的页面不送 LLM，本地合成 low-confidence missing 判定
        #[arg(long)]
        skip_no_fact_pages: bool,
    },
    /// M1:LLM 阅读符号源码,产出/更新 SymbolDoc 知识文档
    KnowledgeScanSymbols {
        /// 游戏脚本根目录(当前树或快照目录)
        root: PathBuf,
        /// 符号类别:component | brain | behaviour(默认 component)
        #[arg(long, default_value = "component")]
        category: String,
        /// 知识文档根目录(默认 knowledge/)
        #[arg(long, default_value = "knowledge")]
        knowledge_dir: PathBuf,
        /// wiki 语料 host 根目录;提供则启用 pass2 语料归因(link-wiki)
        #[arg(long)]
        corpus: Option<PathBuf>,
        /// pass2 每符号采样的页面数
        #[arg(long, default_value_t = 8)]
        sample_pages: usize,
        /// 只处理引用量最高的前 N 个 component
        #[arg(long, default_value_t = 20)]
        limit: usize,
        /// 忽略 sha/prompt_rev 一致性,强制重扫
        #[arg(long)]
        force: bool,
        /// 并行处理的组件数(默认 1 = 串行)
        #[arg(long, default_value_t = 1)]
        concurrency: usize,
        /// 仅对指定文件名词干跑 pass2(逗号分隔);默认全部跑
        #[arg(long, value_delimiter = ',')]
        pass2_names: Option<Vec<String>>,
        /// 仅选取指定文件名词干的符号(逗号分隔);默认按排序取 limit
        #[arg(long, value_delimiter = ',')]
        pick_names: Option<Vec<String>>,
        /// 不调 LLM:仅用 AutoInfobox 冷数据刷新现有 component 文档的 auto_maintained
        #[arg(long)]
        refresh_auto: bool,
        /// pass2 二次确认:采样 ≥3 页但判空时追加一次复查(防过严)
        #[arg(long)]
        confirm_empty: bool,
    },
    /// page-assist:给定页面输出覆盖缺口建议清单(读 PageSymbolMap,不调 LLM)
    PageAssist {
        /// 知识文档根目录(默认 knowledge/)
        #[arg(long, default_value = "knowledge")]
        knowledge_dir: PathBuf,
        /// 页面 id(纯数字)或标题(精确匹配)
        page: Option<String>,
        /// 全库缺口榜(忽略 page 参数)
        #[arg(long)]
        all: bool,
        /// 输出 JSON 而非 Markdown
        #[arg(long)]
        json: bool,
        /// 目标 3:编辑归因模式(区域 × SymbolDoc;需 --corpus)
        #[arg(long)]
        attribute: bool,
        /// wiki 语料根目录(归因模式必需)
        #[arg(long)]
        corpus: Option<PathBuf>,
    },
    /// M3:代码变更 → 脏 SymbolDoc → 页面锚点交叉(确定性;--rescan 级联重扫)
    KnowledgeSync {
        /// 旧快照(时间戳或目录名,SnapshotStore 口径;--review 复核模式可省略)
        old: Option<String>,
        /// 新快照(时间戳、目录名,或 "current" 表示当前 scripts 树)
        #[arg(default_value = "current")]
        new: String,
        /// 知识文档根目录(默认 knowledge/)
        #[arg(long, default_value = "knowledge")]
        knowledge_dir: PathBuf,
        /// 级联重扫脏文档(调 LLM;默认仅输出清单)
        #[arg(long)]
        rescan: bool,
        /// 详列的脏文档数上限
        #[arg(long, default_value_t = 20)]
        limit: usize,
        /// wiki 语料根目录;提供则启用 prefab→页面交叉
        #[arg(long)]
        corpus: Option<PathBuf>,
        /// Tier2:起草页面修订建议(需 --corpus 与 LLM 配置)
        #[arg(long)]
        draft: bool,
        /// Tier2 复核:裁决文件路径(对既有报告的建议逐条 approve/reject)
        #[arg(long)]
        review: Option<PathBuf>,
    },
    /// M2a:PageSymbolMap 确定性骨架(路由/反转/数值配对,不调 LLM)
    KnowledgeScanWiki {
        /// 游戏脚本根目录(当前树或快照目录)
        root: PathBuf,
        /// 知识文档根目录(默认 knowledge/)
        #[arg(long, default_value = "knowledge")]
        knowledge_dir: PathBuf,
        /// wiki 语料 host 根目录
        #[arg(long)]
        corpus: PathBuf,
        /// M2b:对确定性零证据对跑 LLM 审计(收编 symbol-annotate verdict)
        #[arg(long)]
        audit: bool,
        /// 审计符号词干(逗号分隔,如 inspectable,hauntable);默认按缺口取前 10
        #[arg(long, value_delimiter = ',')]
        audit_symbols: Option<Vec<String>>,
        /// 每符号送审页数上限(按 facts 富裕度排序)
        #[arg(long, default_value_t = 60)]
        audit_max_pages: usize,
        /// LLM 每批页数上限
        #[arg(long, default_value_t = 20)]
        audit_batch_pages: usize,
        /// LLM 每批字符数上限
        #[arg(long, default_value_t = 24_000)]
        audit_batch_max_chars: usize,
        /// M2c:不重建地图,聚合现有 knowledge/pages 出报表(summary.json)
        #[arg(long)]
        report: bool,
        /// M2 收尾:语义不一致对三分类(确定性)
        #[arg(long)]
        classify: bool,
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
            Commands::SkillTreeWiki { .. } => "skilltree-wiki",
            Commands::SkillTreeExport { .. } => "skilltree-export",
            Commands::PrefabOverrides { .. } => "prefab-overrides",
            Commands::ScriptsSync { .. } => "scripts-sync",
            Commands::ImagesSync { .. } => "images-sync",
            Commands::AnimSync { .. } => "anim-sync",
            Commands::AnimDiff { .. } => "anim-diff",
            Commands::AnimIndex { .. } => "anim-index",
            Commands::CorpusFetch { .. } => "corpus-fetch",
            Commands::UpdateIndex { .. } => "update-index",
            Commands::UpdateScan { .. } => "update-scan",
            Commands::CorpusIndex { .. } => "corpus-index",
            Commands::SymbolAnnotate { .. } => "symbol-annotate",
            Commands::KnowledgeScanSymbols { .. } => "knowledge-scan-symbols",
            Commands::PageAssist { .. } => "page-assist",
            Commands::KnowledgeSync { .. } => "knowledge-sync",
            Commands::KnowledgeScanWiki { .. } => "knowledge-scan-wiki",
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
    fn test_scripts_sync_command_defaults() {
        let args = Args::try_parse_from(["dst-huiji-wiki", "scripts-sync"]).unwrap();
        match args.command {
            Commands::ScriptsSync {
                force,
                dry_run,
                state,
                report_json,
            } => {
                assert!(!force);
                assert!(!dry_run);
                assert!(state.is_none());
                assert!(report_json.is_none());
            }
            _ => panic!("Expected ScriptsSync command"),
        }
    }

    #[test]
    fn test_scripts_sync_command_with_flags() {
        let args = Args::try_parse_from([
            "dst-huiji-wiki",
            "scripts-sync",
            "--force",
            "--dry-run",
            "--state",
            "ver.txt",
            "--report-json",
            "report.json",
        ])
        .unwrap();
        match args.command {
            Commands::ScriptsSync {
                force,
                dry_run,
                state,
                report_json,
            } => {
                assert!(force);
                assert!(dry_run);
                assert_eq!(state, Some(PathBuf::from("ver.txt")));
                assert_eq!(report_json, Some(PathBuf::from("report.json")));
            }
            _ => panic!("Expected ScriptsSync command"),
        }
    }

    #[test]
    fn test_images_sync_command_defaults() {
        let args = Args::try_parse_from(["dst-huiji-wiki", "images-sync"]).unwrap();
        match args.command {
            Commands::ImagesSync {
                force,
                dry_run,
                report_json,
            } => {
                assert!(!force);
                assert!(!dry_run);
                assert!(report_json.is_none());
            }
            _ => panic!("Expected ImagesSync command"),
        }
    }

    #[test]
    fn test_images_sync_command_with_flags() {
        let args = Args::try_parse_from([
            "dst-huiji-wiki",
            "images-sync",
            "--force",
            "--dry-run",
            "--report-json",
            "report.json",
        ])
        .unwrap();
        match args.command {
            Commands::ImagesSync {
                force,
                dry_run,
                report_json,
            } => {
                assert!(force);
                assert!(dry_run);
                assert_eq!(report_json, Some(PathBuf::from("report.json")));
            }
            _ => panic!("Expected ImagesSync command"),
        }
    }

    #[test]
    fn test_anim_sync_command_defaults() {
        let args = Args::try_parse_from(["dst-huiji-wiki", "anim-sync"]).unwrap();
        match args.command {
            Commands::AnimSync {
                force,
                dry_run,
                label,
                out,
                report_json,
            } => {
                assert!(!force);
                assert!(!dry_run);
                assert!(label.is_none());
                assert!(out.is_none());
                assert!(report_json.is_none());
            }
            _ => panic!("Expected AnimSync command"),
        }
    }

    #[test]
    fn test_anim_diff_command() {
        let args = Args::try_parse_from([
            "dst-huiji-wiki",
            "anim-diff",
            "old_anim",
            "new_anim",
            "--zip",
            "dynamic/abigail_ice.dyn",
            "--report-json",
            "anim-diff.json",
        ])
        .unwrap();
        match args.command {
            Commands::AnimDiff {
                old,
                new,
                zip,
                report_json,
            } => {
                assert_eq!(old, PathBuf::from("old_anim"));
                assert_eq!(new, PathBuf::from("new_anim"));
                assert_eq!(zip, Some("dynamic/abigail_ice.dyn".to_string()));
                assert_eq!(report_json, Some(PathBuf::from("anim-diff.json")));
            }
            _ => panic!("Expected AnimDiff command"),
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
    fn test_symbol_annotate_command() {
        let args = Args::try_parse_from([
            "dst-huiji-wiki",
            "symbol-annotate",
            "scripts",
            "--corpus",
            "wikis/dontstarve.huijiwiki.com",
            "--limit",
            "10",
            "--out",
            "output/symbol-annotate/test",
            "--verdicts",
            "verdicts.json",
            "--llm",
            "--batch-pages",
            "25",
            "--batch-max-chars",
            "16000",
            "--skip-no-fact-pages",
        ]);
        assert!(args.is_ok());
        let args = args.unwrap();
        match args.command {
            Commands::SymbolAnnotate {
                root,
                corpus,
                limit,
                out,
                verdicts,
                llm,
                batch_pages,
                batch_max_chars,
                skip_no_fact_pages,
            } => {
                assert_eq!(root, PathBuf::from("scripts"));
                assert_eq!(corpus, PathBuf::from("wikis/dontstarve.huijiwiki.com"));
                assert_eq!(limit, 10);
                assert_eq!(out, Some(PathBuf::from("output/symbol-annotate/test")));
                assert_eq!(verdicts, Some(PathBuf::from("verdicts.json")));
                assert!(llm);
                assert_eq!(batch_pages, 25);
                assert_eq!(batch_max_chars, 16000);
                assert!(skip_no_fact_pages);
            }
            _ => panic!("Expected SymbolAnnotate command"),
        }

        // knowledge-scan-symbols wiring
        let args = Args::try_parse_from([
            "dst-huiji-wiki",
            "knowledge-scan-symbols",
            "scripts",
            "--corpus",
            "wikis/dontstarve.huijiwiki.com",
            "--sample-pages",
            "12",
            "--limit",
            "3",
            "--force",
        ]);
        match args.unwrap().command {
            Commands::KnowledgeScanSymbols {
                root,
                category,
                knowledge_dir,
                corpus,
                sample_pages,
                limit,
                force,
                concurrency,
                pass2_names,
                pick_names,
                refresh_auto,
                confirm_empty,
            } => {
                assert_eq!(root, PathBuf::from("scripts"));
                assert_eq!(category, "component");
                assert_eq!(knowledge_dir, PathBuf::from("knowledge"));
                assert_eq!(
                    corpus,
                    Some(PathBuf::from("wikis/dontstarve.huijiwiki.com"))
                );
                assert_eq!(sample_pages, 12);
                assert_eq!(limit, 3);
                assert!(force);
                assert_eq!(concurrency, 1);
                assert_eq!(pass2_names, None);
                assert_eq!(pick_names, None);
                assert!(!refresh_auto);
                assert!(!confirm_empty);
            }
            _ => panic!("Expected KnowledgeScanSymbols command"),
        }

        // M2a 命令解析
        let args = Args::try_parse_from([
            "dst-huiji-wiki",
            "knowledge-scan-wiki",
            "scripts",
            "--corpus",
            "wikis/dontstarve.huijiwiki.com",
        ]);
        match args.unwrap().command {
            Commands::KnowledgeScanWiki {
                root,
                knowledge_dir,
                corpus,
                ..
            } => {
                assert_eq!(root, PathBuf::from("scripts"));
                assert_eq!(knowledge_dir, PathBuf::from("knowledge"));
                assert_eq!(corpus, PathBuf::from("wikis/dontstarve.huijiwiki.com"));
            }
            _ => panic!("Expected KnowledgeScanWiki command"),
        }

        // 新类别 flag 透传 + pass2 抽样名单
        let args = Args::try_parse_from([
            "dst-huiji-wiki",
            "knowledge-scan-symbols",
            "scripts",
            "--category",
            "brain",
            "--limit",
            "3",
            "--pass2-names",
            "wander,chaseandattack",
        ]);
        match args.unwrap().command {
            Commands::KnowledgeScanSymbols {
                category,
                pass2_names,
                ..
            } => {
                assert_eq!(category, "brain");
                assert_eq!(
                    pass2_names,
                    Some(vec!["wander".to_string(), "chaseandattack".to_string()])
                );
            }
            _ => panic!("Expected KnowledgeScanSymbols command"),
        }

        // Default: batching falls back to 40 pages per LLM request.
        let args = Args::try_parse_from([
            "dst-huiji-wiki",
            "symbol-annotate",
            "scripts",
            "--corpus",
            "wikis/dontstarve.huijiwiki.com",
        ]);
        match args.unwrap().command {
            Commands::SymbolAnnotate {
                batch_pages,
                batch_max_chars,
                ..
            } => {
                assert_eq!(batch_pages, 40);
                assert_eq!(
                    batch_max_chars,
                    dst_huiji_wiki::update::DEFAULT_BATCH_MAX_CHARS
                );
            }
            _ => panic!("Expected SymbolAnnotate command"),
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
    fn test_skilltree_export_command() {
        let args = Args::try_parse_from([
            "dst-huiji-wiki",
            "skilltree-export",
            "-c",
            "walter",
            "-o",
            "output/skilltree",
            "--report-json",
            "report.json",
        ]);
        assert!(args.is_ok());
        let args = args.unwrap();
        match args.command {
            Commands::SkillTreeExport {
                character,
                output,
                snapshot,
                report_json,
            } => {
                assert_eq!(character, Some("walter".to_string()));
                assert_eq!(output, Some(PathBuf::from("output/skilltree")));
                assert!(snapshot.is_none());
                assert_eq!(report_json, Some(PathBuf::from("report.json")));
            }
            _ => panic!("Expected SkillTreeExport command"),
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
