//! CLI：子命令与参数从 [`JOB_SPECS`]（JobSpec 注册表）+ 本模块的 CLI 视角
//! 元数据生成（clap builder）。
//!
//! 分工：
//! - `service::job_spec`：数据模型层——参数键/类型/默认值/必填（Web JSON
//!   与 CLI 共用，契约测试对齐）；
//! - 本模块 [`CLI_EXT`]：CLI 视角——定位参数、短旗标、旗标重命名/取反、
//!   about 文案、`--yes/--dry-run/--report-json` 等跨切面旗标。
//!
//! 加新 Job：在 `job_spec.rs` 加一条 spec + 在 [`CLI_EXT`] 加一条 CLI 视角
//! + 在 `job_from_matches` 加一个构造臂；三处由契约测试互相锁定。

use clap::{Arg, ArgAction, Command};
use dst_huiji_wiki::platform::progress::{ConfirmMode, StdoutReporter, WriteMode};
use dst_huiji_wiki::service::job_spec::{ParamDefault, ParamKind, ParamSpec};
use dst_huiji_wiki::service::{execute_job_with_mode, JobKind, JOB_SPECS};
use dst_huiji_wiki::Result;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------
// CLI 视角元数据
// ---------------------------------------------------------------------------

/// 旗标特例：CLI 长旗标名与 spec 参数键不同，或取反写入。
struct FlagRename {
    key: &'static str,
    long: &'static str,
    /// CLI 旗标为 true 时向参数写入取反值（`--include-existing` →
    /// `only_missing = false`）。
    invert: bool,
}

const NO_RENAMES: &[FlagRename] = &[];

/// 一个 Job 的 CLI 视角。
struct CliExt {
    /// == `JobSpec.name`（契约测试强制）。
    name: &'static str,
    about: &'static str,
    /// 定位参数（spec key，按顺序）：(key, required)。
    positionals: &'static [(&'static str, bool)],
    /// 使用短旗标的选项：(key, short)。
    shorts: &'static [(&'static str, char)],
    /// 旗标重命名/取反。
    renames: &'static [FlagRename],
    /// 逗号分隔多值（`--x a,b`，可重复出现 `--x a --x b`）。
    comma_lists: &'static [&'static str],
    /// 是否有 `--yes/--dry-run` 写入策略旗标（仅 wiki 写作业）。
    write_args: bool,
    /// 是否有 `--report-json`。
    report_json: bool,
    /// 本地作业的 `--dry-run`（corpus 系：抑制本地写盘 → WriteMode::DryRun）。
    local_dry_run: bool,
}

static CLI_EXT: &[CliExt] = &[
    CliExt {
        name: "parse-po",
        about: "解析 PO 翻译文件为 wiki JSON",
        positionals: &[],
        shorts: &[("input", 'i'), ("output", 'o'), ("category", 'c')],
        renames: NO_RENAMES,
        comma_lists: &[],
        write_args: false,
        report_json: false,
        local_dry_run: false,
    },
    CliExt {
        name: "map-names",
        about: "把 PO 物品名映射为 wiki JSON",
        positionals: &[],
        shorts: &[("input", 'i'), ("output", 'o'), ("compare", 'c'), ("merge", 'm'), ("version", 'v')],
        renames: NO_RENAMES,
        comma_lists: &[],
        write_args: false,
        report_json: false,
        local_dry_run: false,
    },
    CliExt {
        name: "map-recipes",
        about: "把游戏配方映射为 wiki JSON",
        positionals: &[],
        shorts: &[("input", 'i'), ("output", 'o'), ("compare", 'c'), ("merge", 'm'), ("version", 'v')],
        renames: NO_RENAMES,
        comma_lists: &[],
        write_args: false,
        report_json: false,
        local_dry_run: false,
    },
    CliExt {
        name: "maintain-item-table",
        about: "维护 模块:ItemTable（对比线上数据后写维基）",
        positionals: &[],
        shorts: &[("output", 'o')],
        renames: NO_RENAMES,
        comma_lists: &[],
        write_args: true,
        report_json: true,
        local_dry_run: false,
    },
    CliExt {
        name: "maintain-dst-recipes",
        about: "维护 模块:DstRecipes（对比线上数据后写维基）",
        positionals: &[],
        shorts: &[("output", 'o')],
        renames: NO_RENAMES,
        comma_lists: &[],
        write_args: true,
        report_json: true,
        local_dry_run: false,
    },
    CliExt {
        name: "maintain-copy-clip",
        about: "维护模块常量（rbtl/tech/filters/names，标记区间替换）",
        positionals: &[],
        shorts: &[("output", 'o'), ("type", 't')],
        renames: NO_RENAMES,
        comma_lists: &[],
        write_args: true,
        report_json: true,
        local_dry_run: false,
    },
    CliExt {
        name: "maintain-template-check",
        about: "只读检查 模板:Tech/dst 与 模板:制作栏图标 对游戏数据的覆盖率（可选把可粘贴片段写入 --output；不写维基）",
        positionals: &[],
        shorts: &[("output", 'o')],
        renames: NO_RENAMES,
        comma_lists: &[],
        write_args: false,
        report_json: true,
        local_dry_run: false,
    },
    CliExt {
        name: "maintain-strings",
        about: "解析游戏 strings.pot / chinese_s.po，生成 模块:<V> Strings CN/EN <NN> 桶页与 Data:<V>_Strings_Index.json；逐桶语义对比后只写变化页，索引最后写",
        positionals: &[],
        shorts: &[("output", 'o')],
        renames: NO_RENAMES,
        comma_lists: &[],
        write_args: true,
        report_json: true,
        local_dry_run: false,
    },
    CliExt {
        name: "skilltree-wiki",
        about: "把游戏 skilltree_<char>.lua 提取为 模块:Skilltree/<Char> 子页面的 defs JSON 并维护维基子页面（保留页内 metainfo/icon_url；--output 同时写出 Skilltree.js 渲染器与图片清单）",
        positionals: &[],
        shorts: &[("character", 'c'), ("output", 'o')],
        renames: NO_RENAMES,
        comma_lists: &[],
        write_args: true,
        report_json: true,
        local_dry_run: false,
    },
    CliExt {
        name: "skilltree-export",
        about: "把技能树数据导出为本地 JSON 文件（纯本地，不访问维基）",
        positionals: &[],
        shorts: &[("character", 'c'), ("output", 'o')],
        renames: NO_RENAMES,
        comma_lists: &[],
        write_args: false,
        report_json: true,
        local_dry_run: false,
    },
    CliExt {
        name: "upload-image",
        about: "上传本地图片到维基（同名重传请加 --ignore-warnings；按 skilltree/ skilltree_icons/ inventoryimages/ 目录自动补分类描述）",
        positionals: &[("path", true)],
        shorts: &[],
        renames: NO_RENAMES,
        comma_lists: &[],
        write_args: true,
        report_json: true,
        local_dry_run: false,
    },
    CliExt {
        name: "prefab-overrides",
        about: "解析单个 Lua 文件的预制体重定向",
        positionals: &[],
        shorts: &[("input", 'i'), ("output", 'o')],
        renames: NO_RENAMES,
        comma_lists: &[],
        write_args: false,
        report_json: false,
        local_dry_run: false,
    },
    CliExt {
        name: "prefab-overrides-dir",
        about: "扫描目录下全部 Lua 文件解析预制体重定向并合并（纯本地操作；目录缺省为 DST__ROOT/data/databundles/scripts/prefabs）",
        positionals: &[],
        shorts: &[("input", 'i'), ("output", 'o')],
        renames: NO_RENAMES,
        comma_lists: &[],
        write_args: false,
        report_json: false,
        local_dry_run: false,
    },
    CliExt {
        name: "prefab-overrides-audit",
        about: "只读审计 模块:ItemTable/PrefabOverrides：线上条目 × 本地推导做 key/target 存在性验证，输出可推导/需人工补丁/可疑三张清单",
        positionals: &[],
        shorts: &[("output", 'o')],
        renames: NO_RENAMES,
        comma_lists: &[],
        write_args: false,
        report_json: false,
        local_dry_run: false,
    },
    CliExt {
        name: "maintain-prefab-overrides",
        about: "维护 模块:ItemTable/PrefabOverrides（只读 diff，不写维基）：本地推导与线上页面合并对比，产出更新/新增清单与本地页面文本",
        positionals: &[],
        shorts: &[("output", 'o')],
        renames: NO_RENAMES,
        comma_lists: &[],
        write_args: false,
        report_json: false,
        local_dry_run: false,
    },
    CliExt {
        name: "scripts-sync",
        about: "同步更新后的游戏 scripts:归档旧树为快照、解压新 scripts.zip(纯本地操作)",
        positionals: &[],
        shorts: &[],
        // `--state`（CLI 习惯名）写入 serde 的 `state_path`。
        renames: &[FlagRename { key: "state_path", long: "state", invert: false }],
        comma_lists: &[],
        write_args: false,
        report_json: true,
        local_dry_run: false,
    },
    CliExt {
        name: "images-sync",
        about: "处理游戏图片资源:解压 images.zip、ktech 解码、按 xml 切割(纯本地图片操作),结束后刷新图标元数据并查询维基上传状态(可用 --skip-wiki-status 跳过)",
        positionals: &[],
        shorts: &[],
        renames: NO_RENAMES,
        comma_lists: &[],
        write_args: false,
        report_json: true,
        local_dry_run: false,
    },
    CliExt {
        name: "upload-icons",
        about: "上传图标到维基（物品栏/制作栏/技能树；标题按来源自动生成或取映射表，描述写对应分类；批量默认仅上传维基缺失的，需先运行 images-sync；--file + --title 手动指定站内文件名）",
        positionals: &[],
        shorts: &[],
        renames: &[FlagRename { key: "only_missing", long: "include-existing", invert: true }],
        comma_lists: &[],
        write_args: true,
        report_json: true,
        local_dry_run: false,
    },
    CliExt {
        name: "anim-sync",
        about: "动画历史同步:更新后扫描 data/anim，归档 zip/dyn 到 ANIM__OUT_DIR",
        positionals: &[],
        shorts: &[],
        renames: NO_RENAMES,
        comma_lists: &[],
        write_args: false,
        report_json: true,
        local_dry_run: false,
    },
    CliExt {
        name: "anim-diff",
        about: "对比两个动画目录的 zip/dyn 结构化 diff",
        positionals: &[("old", true), ("new", true)],
        shorts: &[],
        renames: NO_RENAMES,
        comma_lists: &[],
        write_args: false,
        report_json: true,
        local_dry_run: false,
    },
    CliExt {
        name: "anim-index",
        about: "构建 prefab 与动画文件的关联索引（纯本地，不做皮肤关联）",
        positionals: &[("scripts", true)],
        shorts: &[("out", 'o')],
        renames: NO_RENAMES,
        comma_lists: &[],
        write_args: false,
        report_json: true,
        local_dry_run: false,
    },
    CliExt {
        name: "skin-index",
        about: "解析 skinprefabs.lua 生成 base_prefab → skins 皮肤索引（纯本地）",
        positionals: &[],
        shorts: &[("out", 'o')],
        renames: NO_RENAMES,
        comma_lists: &[],
        write_args: false,
        report_json: true,
        local_dry_run: false,
    },
    CliExt {
        name: "corpus-fetch",
        about: "抓取维基主命名空间全量语料到本地目录（默认 wikis/，不入仓库）；唯一允许匿名读的作业",
        positionals: &[],
        shorts: &[],
        renames: NO_RENAMES,
        comma_lists: &[],
        write_args: false,
        report_json: false,
        local_dry_run: true,
    },
    CliExt {
        name: "update-scan",
        about: "快照差异 + 关联影响评估（M1，只读）：产出 impact.json 与 changes.patch",
        positionals: &[("old", true), ("new", true)],
        shorts: &[("out", 'o')],
        renames: NO_RENAMES,
        comma_lists: &[],
        write_args: false,
        report_json: false,
        local_dry_run: false,
    },
    CliExt {
        name: "update-index",
        about: "构建代码关联索引（基础设施A）并缓存到 output/atlas/<build>/",
        positionals: &[("root", true)],
        shorts: &[("out", 'o')],
        renames: NO_RENAMES,
        comma_lists: &[],
        write_args: false,
        report_json: false,
        local_dry_run: false,
    },
    CliExt {
        name: "corpus-index",
        about: "从本地语料树重建派生索引（prefab 注册表等，纯本地操作）",
        positionals: &[],
        shorts: &[],
        renames: NO_RENAMES,
        comma_lists: &[],
        write_args: false,
        report_json: false,
        local_dry_run: true,
    },
    CliExt {
        name: "symbol-annotate",
        about: "Page→Symbol 标注 CLI：生成高引用 symbol 证据包和 Prompt，可选读取已有 LLM 输出并生成跨页一致性报告（纯本地，不写 wiki）",
        positionals: &[("root", true)],
        shorts: &[],
        renames: NO_RENAMES,
        comma_lists: &[],
        write_args: false,
        report_json: false,
        local_dry_run: false,
    },
    CliExt {
        name: "knowledge-scan-symbols",
        about: "M1:LLM 阅读符号源码,产出/更新 SymbolDoc 知识文档",
        positionals: &[("root", true)],
        shorts: &[],
        renames: NO_RENAMES,
        comma_lists: &["pass2_names", "pick_names"],
        write_args: false,
        report_json: false,
        local_dry_run: false,
    },
    CliExt {
        name: "page-assist",
        about: "page-assist:给定页面输出覆盖缺口建议清单(读 PageSymbolMap,不调 LLM)",
        positionals: &[("page", false)],
        shorts: &[],
        renames: NO_RENAMES,
        comma_lists: &[],
        write_args: false,
        report_json: false,
        local_dry_run: false,
    },
    CliExt {
        name: "knowledge-sync",
        about: "M3:代码变更 → 脏 SymbolDoc → 页面锚点交叉(确定性;--rescan 级联重扫)",
        // `old` 在 --review 复核模式下可省略；`new` 有默认值 "current"。
        positionals: &[("old", false), ("new", false)],
        shorts: &[],
        renames: NO_RENAMES,
        comma_lists: &[],
        write_args: false,
        report_json: false,
        local_dry_run: false,
    },
    CliExt {
        name: "knowledge-scan-wiki",
        about: "M2a:PageSymbolMap 确定性骨架(路由/反转/数值配对,不调 LLM)",
        positionals: &[("root", true)],
        shorts: &[],
        renames: NO_RENAMES,
        comma_lists: &["audit_symbols"],
        write_args: false,
        report_json: false,
        local_dry_run: false,
    },
    CliExt {
        name: "maintain-wikitext",
        about: "用 wikitext 解析器批量修改页面里指定模板的参数（外科手术式编辑，未修改部分逐字节还原；--dry-run 只产 diff 报告不写维基）",
        positionals: &[],
        shorts: &[("output", 'o')],
        renames: &[FlagRename { key: "pages", long: "page", invert: false }],
        comma_lists: &["pages", "set", "remove"],
        write_args: true,
        report_json: true,
        local_dry_run: false,
    },
    CliExt {
        name: "create-redirect",
        about: "在 wiki 上创建页面重定向（#REDIRECT [[目标]]；--dry-run 只预览 wikitext）",
        positionals: &[("from", true), ("to", true)],
        shorts: &[],
        renames: NO_RENAMES,
        comma_lists: &[],
        write_args: true,
        report_json: true,
        local_dry_run: false,
    },
    CliExt {
        name: "cooking-game-export",
        about: "把料理模拟数据、图标与 example_combo 打包给独立烹饪小游戏项目",
        positionals: &[],
        shorts: &[("output", 'o')],
        renames: NO_RENAMES,
        comma_lists: &[],
        write_args: false,
        report_json: true,
        local_dry_run: false,
    },
];

fn ext(name: &str) -> &'static CliExt {
    CLI_EXT
        .iter()
        .find(|e| e.name == name)
        .unwrap_or_else(|| panic!("CLI_EXT 缺少 {name}"))
}

// ---------------------------------------------------------------------------
// clap 命令构建
// ---------------------------------------------------------------------------

/// 构建完整 CLI（顶层 + serve + 34 个 job 子命令）。
pub fn command() -> Command {
    let mut top = Command::new("dst-huiji-wiki")
        .about("饥荒联机版维基维护工具")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new("serve")
                .about("启动 WebUI 服务器")
                .arg(
                    Arg::new("host")
                        .long("host")
                        .default_value("127.0.0.1")
                        .help("监听地址（默认 127.0.0.1）"),
                )
                .arg(
                    Arg::new("port")
                        .long("port")
                        .default_value("8420")
                        .help("监听端口"),
                ),
        );

    for spec in JOB_SPECS {
        top = top.subcommand(job_subcommand(spec));
    }
    top
}

fn job_subcommand(spec: &dst_huiji_wiki::service::JobSpec) -> Command {
    let ext = ext(spec.name);
    let mut cmd = Command::new(spec.name).about(ext.about);

    for param in spec.params {
        // 定位参数由 positionals 表声明；renames 键在对应 rename 处建旗标。
        if ext.positionals.iter().any(|(k, _)| *k == param.key) {
            continue;
        }
        if ext.renames.iter().any(|r| r.key == param.key) {
            continue;
        }
        cmd = cmd.arg(spec_option_arg(param, ext, param.key, param.key, false));
    }

    for (key, required) in ext.positionals {
        let param = spec
            .param(key)
            .unwrap_or_else(|| panic!("spec {} 缺少定位参数 {key}", spec.name));
        let mut arg = Arg::new(*key)
            .help(param.help)
            .action(ArgAction::Set)
            .value_name(key);
        if *required {
            arg = arg.required(true);
        } else if let Some(d) = default_value(param) {
            arg = arg.default_value(d);
        }
        cmd = cmd.arg(arg);
    }

    for rename in ext.renames {
        let param = spec
            .param(rename.key)
            .unwrap_or_else(|| panic!("spec {} 缺少 rename 键 {}", spec.name, rename.key));
        cmd = cmd.arg(spec_option_arg(
            param,
            ext,
            rename.key,
            rename.long,
            rename.invert,
        ));
    }

    if ext.write_args {
        cmd = cmd
            .arg(
                Arg::new("yes")
                    .long("yes")
                    .action(ArgAction::SetTrue)
                    .help("跳过确认，直接写入维基"),
            )
            .arg(
                Arg::new("dry_run")
                    .long("dry-run")
                    .action(ArgAction::SetTrue)
                    .conflicts_with("yes")
                    .help("只生成产物与 diff，不写入维基（与 --yes 互斥）"),
            );
    }
    if ext.local_dry_run {
        cmd = cmd.arg(
            Arg::new("dry_run")
                .long("dry-run")
                .action(ArgAction::SetTrue)
                .help("只枚举与对账出报告，不写任何本地文件"),
        );
    }
    if ext.report_json {
        cmd = cmd.arg(
            Arg::new("report_json")
                .long("report-json")
                .action(ArgAction::Set)
                .value_name("FILE")
                .help("将机器可读的执行报告（JSON）写入该文件"),
        );
    }
    cmd
}

/// 为一个 spec 参数构建 CLI Arg（选项形式）。
fn spec_option_arg(
    param: &ParamSpec,
    ext: &CliExt,
    id: &'static str,
    long: &'static str,
    invert: bool,
) -> Arg {
    let short = ext
        .shorts
        .iter()
        .find(|(k, _)| *k == param.key)
        .map(|(_, c)| *c);

    let kind = if invert { ParamKind::Bool } else { param.kind };

    let mut arg = match kind {
        ParamKind::Bool => {
            let mut a = Arg::new(id)
                .long(long)
                .action(ArgAction::SetTrue)
                .help(param.help);
            if invert {
                a = a.help("连同维基上已存在的图标一起上传（默认只上传缺失的）");
            }
            a
        }
        ParamKind::Usize => {
            let mut a = Arg::new(id)
                .long(long)
                .action(ArgAction::Set)
                .help(param.help);
            if let Some(d) = default_value(param) {
                a = a.default_value(d);
            }
            a
        }
        ParamKind::StrList => {
            // 多值参数（spec.comma/append 语义）：可重复出现；comma_lists
            // 额外允许 `--x a,b` 逗号切分。
            let mut a = Arg::new(id)
                .long(long)
                .action(ArgAction::Append)
                .num_args(1)
                .help(param.help);
            if ext.comma_lists.contains(&param.key) {
                a = a.value_delimiter(',');
            }
            a
        }
        ParamKind::Str | ParamKind::Path | ParamKind::OptStr | ParamKind::OptPath => {
            let mut a = Arg::new(id)
                .long(long)
                .action(ArgAction::Set)
                .help(param.help);
            // 有默认值的 Str 视为可选（clap 不允许 required + default 并存）。
            let has_default = default_value(param).is_some();
            if matches!(param.kind, ParamKind::Str | ParamKind::Path) && !has_default {
                a = a.required(true);
            }
            if let Some(d) = default_value(param) {
                a = a.default_value(d);
            }
            a
        }
    };
    if let Some(c) = short {
        arg = arg.short(c);
    }
    arg
}

fn default_value(param: &ParamSpec) -> Option<&'static str> {
    match param.default {
        ParamDefault::None => None,
        ParamDefault::Bool(_) => None,
        ParamDefault::Int(n) => Some(Box::leak(n.to_string().into_boxed_str())),
        ParamDefault::Str(s) => Some(s),
    }
}

// ---------------------------------------------------------------------------
// 解析
// ---------------------------------------------------------------------------

/// CLI 解析结果。
pub enum TopCommand {
    Serve { host: String, port: u16 },
    Job(Invocation),
}

pub struct Invocation {
    pub kind: JobKind,
    pub mode: WriteMode,
    pub report_json: Option<PathBuf>,
}

/// 解析进程参数（解析错误/--help 由 clap 直接处理进程退出）。
pub fn parse() -> Result<TopCommand> {
    let matches = command().get_matches();
    top_from_matches(&matches)
}

fn top_from_matches(m: &clap::ArgMatches) -> Result<TopCommand> {
    match m.subcommand() {
        Some(("serve", sub)) => Ok(TopCommand::Serve {
            host: sub.get_one::<String>("host").cloned().unwrap_or_default(),
            port: sub
                .get_one::<String>("port")
                .and_then(|p| p.parse().ok())
                .unwrap_or(8420),
        }),
        Some((name, sub)) => {
            let spec = JOB_SPECS
                .iter()
                .find(|s| s.name == name)
                .unwrap_or_else(|| panic!("未知子命令 {name}"));
            let ext = ext(name);
            let kind = job_from_matches(spec, ext, sub)?;

            let mode = if ext.write_args {
                let yes = sub.get_flag("yes");
                let dry_run = sub.get_flag("dry_run");
                if dry_run {
                    WriteMode::DryRun
                } else if yes {
                    WriteMode::AutoConfirm
                } else {
                    WriteMode::Interactive
                }
            } else if ext.local_dry_run && sub.get_flag("dry_run") {
                WriteMode::DryRun
            } else {
                WriteMode::AutoConfirm
            };

            let report_json = if ext.report_json {
                sub.get_one::<String>("report_json")
                    .cloned()
                    .map(PathBuf::from)
            } else {
                None
            };

            Ok(TopCommand::Job(Invocation {
                kind,
                mode,
                report_json,
            }))
        }
        _ => unreachable!("subcommand_required(true)"),
    }
}

// --- ArgMatches 取值辅助 ---

fn val(m: &clap::ArgMatches, key: &str) -> String {
    m.get_one::<String>(key)
        .cloned()
        .unwrap_or_else(|| panic!("参数 {key} 缺失"))
}

fn opt_val(m: &clap::ArgMatches, key: &str) -> Option<String> {
    m.get_one::<String>(key).cloned()
}

fn flag(m: &clap::ArgMatches, key: &str) -> bool {
    m.get_flag(key)
}

fn num(m: &clap::ArgMatches, key: &str) -> usize {
    m.get_one::<String>(key)
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| panic!("参数 {key} 不是非负整数"))
}

fn opt_list(m: &clap::ArgMatches, key: &str) -> Option<Vec<String>> {
    let v: Vec<String> = m.get_many::<String>(key)?.cloned().collect();
    Some(v).filter(|v| !v.is_empty())
}

fn list(m: &clap::ArgMatches, key: &str) -> Vec<String> {
    m.get_many::<String>(key)
        .map(|v| v.cloned().collect())
        .unwrap_or_default()
}

/// 按 spec/CLI 元数据把 ArgMatches 组装成 [`JobKind`]。
fn job_from_matches(
    spec: &dst_huiji_wiki::service::JobSpec,
    _ext: &CliExt,
    m: &clap::ArgMatches,
) -> Result<JobKind> {
    let _ = spec;
    Ok(match spec.name {
        "parse-po" => JobKind::ParsePo {
            input: val(m, "input"),
            output: opt_val(m, "output"),
            category: opt_val(m, "category"),
        },
        "map-names" => JobKind::MapNames {
            input: val(m, "input"),
            output: opt_val(m, "output"),
            compare: opt_val(m, "compare"),
            merge: flag(m, "merge"),
            version: opt_val(m, "version"),
        },
        "map-recipes" => JobKind::MapRecipes {
            input: val(m, "input"),
            output: opt_val(m, "output"),
            compare: opt_val(m, "compare"),
            merge: flag(m, "merge"),
            po_file: opt_val(m, "po_file"),
            version: opt_val(m, "version"),
        },
        "maintain-item-table" => JobKind::MaintainItemTable {
            output: opt_val(m, "output"),
            snapshot: opt_val(m, "snapshot"),
        },
        "maintain-dst-recipes" => JobKind::MaintainDstRecipes {
            output: opt_val(m, "output"),
            snapshot: opt_val(m, "snapshot"),
        },
        "maintain-copy-clip" => JobKind::MaintainCopyClip {
            r#type: opt_val(m, "type"),
            output: opt_val(m, "output"),
            snapshot: opt_val(m, "snapshot"),
        },
        "maintain-template-check" => JobKind::MaintainTemplateCheck {
            output: opt_val(m, "output"),
            snapshot: opt_val(m, "snapshot"),
            skip_icon_status: flag(m, "skip_icon_status"),
        },
        "maintain-strings" => JobKind::MaintainStrings {
            version: val(m, "version"),
            snapshot: opt_val(m, "snapshot"),
            output: opt_val(m, "output"),
            offline: flag(m, "offline"),
            bucket_count: num(m, "bucket_count"),
            rebalance: flag(m, "rebalance"),
            limit: num(m, "limit"),
        },
        "skilltree-wiki" => JobKind::SkilltreeWiki {
            character: opt_val(m, "character"),
            output: opt_val(m, "output"),
            snapshot: opt_val(m, "snapshot"),
        },
        "skilltree-export" => JobKind::SkilltreeExport {
            character: opt_val(m, "character"),
            output: opt_val(m, "output"),
            snapshot: opt_val(m, "snapshot"),
        },
        "upload-image" => JobKind::UploadImage {
            path: val(m, "path"),
            name: opt_val(m, "name"),
            description: opt_val(m, "description"),
            comment: opt_val(m, "comment"),
            ignore_warnings: flag(m, "ignore_warnings"),
        },
        "prefab-overrides" => JobKind::PrefabOverrides {
            input: val(m, "input"),
            output: opt_val(m, "output"),
        },
        "prefab-overrides-dir" => JobKind::PrefabOverridesDir {
            input: opt_val(m, "input"),
            output: opt_val(m, "output"),
        },
        "prefab-overrides-audit" => JobKind::PrefabOverridesAudit {
            scripts: opt_val(m, "scripts"),
            wiki_file: opt_val(m, "wiki_file"),
            output: opt_val(m, "output"),
        },
        "maintain-prefab-overrides" => JobKind::MaintainPrefabOverrides {
            scripts: opt_val(m, "scripts"),
            wiki_file: opt_val(m, "wiki_file"),
            output: opt_val(m, "output"),
        },
        "scripts-sync" => JobKind::ScriptsSync {
            force: flag(m, "force"),
            dry_run: flag(m, "dry_run"),
            state_path: opt_val(m, "state_path"),
        },
        "images-sync" => JobKind::ImagesSync {
            force: flag(m, "force"),
            dry_run: flag(m, "dry_run"),
            skip_wiki_status: flag(m, "skip_wiki_status"),
        },
        "upload-icons" => JobKind::UploadIcons {
            build: opt_val(m, "build"),
            source: opt_val(m, "source"),
            file: opt_val(m, "file"),
            title: opt_val(m, "title"),
            only_missing: !flag(m, "only_missing"),
            ignore_warnings: flag(m, "ignore_warnings"),
            comment: opt_val(m, "comment"),
        },
        "anim-sync" => JobKind::AnimSync {
            force: flag(m, "force"),
            dry_run: flag(m, "dry_run"),
            label: opt_val(m, "label"),
            out: opt_val(m, "out"),
        },
        "anim-diff" => JobKind::AnimDiff {
            old: val(m, "old"),
            new: val(m, "new"),
            zip: opt_val(m, "zip"),
        },
        "anim-index" => JobKind::AnimIndex {
            scripts: val(m, "scripts"),
            anim: opt_val(m, "anim"),
            out: opt_val(m, "out"),
        },
        "skin-index" => JobKind::SkinIndex {
            scripts: opt_val(m, "scripts"),
            anim: opt_val(m, "anim"),
            out: opt_val(m, "out"),
        },
        "update-scan" => JobKind::UpdateScan {
            old: val(m, "old"),
            new: val(m, "new"),
            out: opt_val(m, "out"),
            corpus: opt_val(m, "corpus"),
            annotate: opt_val(m, "annotate"),
        },
        "update-index" => JobKind::UpdateIndex {
            root: val(m, "root"),
            out: opt_val(m, "out"),
        },
        "corpus-fetch" => JobKind::CorpusFetch {
            full: flag(m, "full"),
            dir: opt_val(m, "dir"),
            rc: flag(m, "rc"),
        },
        "corpus-index" => JobKind::CorpusIndex {
            dir: opt_val(m, "dir"),
            join: opt_val(m, "join"),
        },
        "symbol-annotate" => JobKind::SymbolAnnotate {
            root: val(m, "root"),
            corpus: val(m, "corpus"),
            limit: num(m, "limit"),
            out: opt_val(m, "out"),
            verdicts: opt_val(m, "verdicts"),
            llm: flag(m, "llm"),
            batch_pages: num(m, "batch_pages"),
            batch_max_chars: num(m, "batch_max_chars"),
            skip_no_fact_pages: flag(m, "skip_no_fact_pages"),
        },
        "knowledge-scan-symbols" => JobKind::KnowledgeScanSymbols {
            root: val(m, "root"),
            category: val(m, "category"),
            knowledge_dir: val(m, "knowledge_dir"),
            corpus: opt_val(m, "corpus"),
            sample_pages: num(m, "sample_pages"),
            limit: num(m, "limit"),
            force: flag(m, "force"),
            concurrency: num(m, "concurrency"),
            pass2_names: opt_list(m, "pass2_names"),
            pick_names: opt_list(m, "pick_names"),
            refresh_auto: flag(m, "refresh_auto"),
            confirm_empty: flag(m, "confirm_empty"),
        },
        "page-assist" => JobKind::PageAssist {
            knowledge_dir: val(m, "knowledge_dir"),
            page: opt_val(m, "page"),
            all: flag(m, "all"),
            json: flag(m, "json"),
            attribute: flag(m, "attribute"),
            corpus: opt_val(m, "corpus"),
        },
        "knowledge-sync" => JobKind::KnowledgeSync {
            old: opt_val(m, "old").unwrap_or_default(),
            new: opt_val(m, "new").unwrap_or_else(|| "current".to_string()),
            knowledge_dir: val(m, "knowledge_dir"),
            rescan: flag(m, "rescan"),
            limit: num(m, "limit"),
            corpus: opt_val(m, "corpus"),
            draft: flag(m, "draft"),
            review: opt_val(m, "review"),
        },
        "knowledge-scan-wiki" => JobKind::KnowledgeScanWiki {
            root: val(m, "root"),
            knowledge_dir: val(m, "knowledge_dir"),
            corpus: val(m, "corpus"),
            audit: flag(m, "audit"),
            audit_symbols: opt_list(m, "audit_symbols"),
            audit_max_pages: num(m, "audit_max_pages"),
            audit_batch_pages: num(m, "audit_batch_pages"),
            audit_batch_max_chars: num(m, "audit_batch_max_chars"),
            report: flag(m, "report"),
            classify: flag(m, "classify"),
        },
        "maintain-wikitext" => JobKind::MaintainWikitext {
            pages: list(m, "pages"),
            template: val(m, "template"),
            set: list(m, "set"),
            remove: list(m, "remove"),
            output: opt_val(m, "output"),
        },
        "create-redirect" => JobKind::CreateRedirect {
            from: val(m, "from"),
            to: val(m, "to"),
            summary: opt_val(m, "summary"),
        },
        "cooking-game-export" => JobKind::CookingGameExport {
            output: opt_val(m, "output"),
            snapshot: opt_val(m, "snapshot"),
            zip: flag(m, "zip"),
            allow_missing_icons: flag(m, "allow_missing_icons"),
        },
        other => unreachable!("未实现构造臂的 job: {other}"),
    })
}

// ---------------------------------------------------------------------------
// 执行
// ---------------------------------------------------------------------------

pub async fn run(top: TopCommand) -> Result<()> {
    match top {
        TopCommand::Serve { host, port } => crate::web::serve(host, port).await?,
        TopCommand::Job(inv) => {
            // AutoConfirm/DryRun never consult the reporter (decide_write
            // short-circuits), so stdin prompting stays Interactive-only.
            let reporter = StdoutReporter {
                confirm: ConfirmMode::Interactive,
            };
            let result = execute_job_with_mode(&inv.kind, &reporter, inv.mode).await?;

            if let Some(path) = inv.report_json {
                let now_ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);
                // result 已是 report schema v1 外壳（wrap_report），
                // 文件层再附加生成环境元数据。
                let mut report = result;
                if let Some(obj) = report.as_object_mut() {
                    obj.insert("generated_at_ms".into(), serde_json::json!(now_ms));
                    obj.insert(
                        "params".into(),
                        serde_json::to_value(&inv.kind).unwrap_or_default(),
                    );
                    obj.insert("write_mode".into(), serde_json::json!(inv.mode.name()));
                }
                dst_huiji_wiki::platform::fs::write_json_atomic(&path, &report)?;
                println!("报告已写入 {:?}", path);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 契约：CLI_EXT 与 JOB_SPECS 一一对应；除跨切面旗标（yes/dry-run/
    /// report-json）与显式 renames 外，CLI 旗标集合 == spec 参数键集合。
    #[test]
    fn contract_cli_flags_match_spec_params() {
        for spec in JOB_SPECS {
            let ext = ext(spec.name);
            let cmd = job_subcommand(spec);
            let cli_args: std::collections::BTreeSet<String> = cmd
                .get_arguments()
                .filter_map(|a| a.get_long().map(str::to_string))
                .collect();

            let mut expected: std::collections::BTreeSet<String> = spec
                .params
                .iter()
                .filter(|p| !ext.positionals.iter().any(|(k, _)| *k == p.key))
                .map(|p| {
                    ext.renames
                        .iter()
                        .find(|r| r.key == p.key)
                        .map(|r| r.long.to_string())
                        .unwrap_or_else(|| p.key.to_string())
                })
                .collect();
            if ext.write_args {
                expected.insert("yes".into());
                expected.insert("dry-run".into());
            }
            if ext.local_dry_run {
                expected.insert("dry-run".into());
            }
            if ext.report_json {
                expected.insert("report-json".into());
            }
            assert_eq!(
                cli_args, expected,
                "spec `{}` 的 CLI 旗标与参数声明不一致",
                spec.name
            );
        }
    }

    fn parse_args(argv: &[&str]) -> TopCommand {
        let matches = command().try_get_matches_from(argv).unwrap();
        top_from_matches(&matches).unwrap()
    }

    #[test]
    fn parse_maintain_copyclip_type_flag() {
        let top = parse_args(&["dst-huiji-wiki", "maintain-copy-clip", "-t", "tech"]);
        let TopCommand::Job(inv) = top else {
            panic!("expected job");
        };
        assert_eq!(inv.kind.name(), "maintain-copy-clip");
        match inv.kind {
            JobKind::MaintainCopyClip { r#type, .. } => {
                assert_eq!(r#type.as_deref(), Some("tech"))
            }
            _ => panic!("wrong variant"),
        }
        assert_eq!(inv.mode, WriteMode::Interactive);
    }

    #[test]
    fn parse_write_mode_flags() {
        let top = parse_args(&["dst-huiji-wiki", "maintain-item-table", "--yes"]);
        let TopCommand::Job(inv) = top else { panic!() };
        assert_eq!(inv.mode, WriteMode::AutoConfirm);

        let top = parse_args(&["dst-huiji-wiki", "maintain-item-table", "--dry-run"]);
        let TopCommand::Job(inv) = top else { panic!() };
        assert_eq!(inv.mode, WriteMode::DryRun);
    }

    #[test]
    fn parse_upload_icons_inverted_flag() {
        let top = parse_args(&[
            "dst-huiji-wiki",
            "upload-icons",
            "--include-existing",
            "--yes",
        ]);
        let TopCommand::Job(inv) = top else { panic!() };
        match inv.kind {
            JobKind::UploadIcons { only_missing, .. } => assert!(!only_missing),
            _ => panic!("wrong variant"),
        }
        // 缺省 only_missing = true（spec 默认）。
        let top = parse_args(&["dst-huiji-wiki", "upload-icons"]);
        let TopCommand::Job(inv) = top else { panic!() };
        match inv.kind {
            JobKind::UploadIcons { only_missing, .. } => assert!(only_missing),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn parse_scripts_sync_state_alias() {
        let top = parse_args(&[
            "dst-huiji-wiki",
            "scripts-sync",
            "--force",
            "--state",
            "/tmp/v.txt",
        ]);
        let TopCommand::Job(inv) = top else { panic!() };
        match inv.kind {
            JobKind::ScriptsSync {
                force, state_path, ..
            } => {
                assert!(force);
                assert_eq!(state_path, Some("/tmp/v.txt".to_string()));
            }
            _ => panic!("wrong variant"),
        }
        assert_eq!(inv.mode, WriteMode::AutoConfirm);
    }

    #[test]
    fn parse_maintain_strings_defaults_from_spec() {
        let top = parse_args(&["dst-huiji-wiki", "maintain-strings"]);
        let TopCommand::Job(inv) = top else { panic!() };
        match inv.kind {
            JobKind::MaintainStrings {
                version,
                bucket_count,
                limit,
                ..
            } => {
                assert_eq!(version, "DST");
                assert_eq!(bucket_count, 100);
                assert_eq!(limit, 0);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn parse_positionals_and_lists() {
        let top = parse_args(&[
            "dst-huiji-wiki",
            "anim-diff",
            "old_dir",
            "new_dir",
            "--zip",
            "dynamic/a.dyn",
        ]);
        let TopCommand::Job(inv) = top else { panic!() };
        match inv.kind {
            JobKind::AnimDiff { old, new, zip } => {
                assert_eq!(old, "old_dir");
                assert_eq!(new, "new_dir");
                assert_eq!(zip.as_deref(), Some("dynamic/a.dyn"));
            }
            _ => panic!("wrong variant"),
        }

        let top = parse_args(&[
            "dst-huiji-wiki",
            "maintain-wikitext",
            "--page",
            "a,b",
            "--page",
            "c",
            "--template",
            "实体信息框/自动",
            "--set",
            "k=1",
        ]);
        let TopCommand::Job(inv) = top else { panic!() };
        match inv.kind {
            JobKind::MaintainWikitext { pages, set, .. } => {
                assert_eq!(pages, vec!["a", "b", "c"]);
                assert_eq!(set, vec!["k=1"]);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn parse_corpus_fetch_local_dry_run() {
        let top = parse_args(&["dst-huiji-wiki", "corpus-fetch", "--dry-run"]);
        let TopCommand::Job(inv) = top else { panic!() };
        assert_eq!(inv.mode, WriteMode::DryRun);
        match inv.kind {
            JobKind::CorpusFetch { full, rc, .. } => {
                assert!(!full);
                assert!(!rc);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn parse_knowledge_sync_optional_old_and_default_new() {
        let top = parse_args(&[
            "dst-huiji-wiki",
            "knowledge-sync",
            "--review",
            "verdicts.json",
        ]);
        let TopCommand::Job(inv) = top else { panic!() };
        match inv.kind {
            JobKind::KnowledgeSync {
                old, new, review, ..
            } => {
                assert_eq!(old, "");
                assert_eq!(new, "current");
                assert_eq!(review, Some("verdicts.json".to_string()));
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn parse_knowledge_scan_symbols_comma_lists() {
        let top = parse_args(&[
            "dst-huiji-wiki",
            "knowledge-scan-symbols",
            "root_dir",
            "--pass2_names",
            "abigail,hound",
            "--limit",
            "5",
        ]);
        let TopCommand::Job(inv) = top else { panic!() };
        match inv.kind {
            JobKind::KnowledgeScanSymbols {
                root,
                pass2_names,
                limit,
                knowledge_dir,
                ..
            } => {
                assert_eq!(root, "root_dir");
                assert_eq!(
                    pass2_names,
                    Some(vec!["abigail".to_string(), "hound".to_string()])
                );
                assert_eq!(limit, 5);
                assert_eq!(knowledge_dir, "knowledge");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn parse_serve_subcommand() {
        let top = parse_args(&["dst-huiji-wiki", "serve", "--port", "9000"]);
        match top {
            TopCommand::Serve { host, port } => {
                assert_eq!(host, "127.0.0.1");
                assert_eq!(port, 9000);
            }
            TopCommand::Job(_) => panic!("expected serve"),
        }
    }

    #[test]
    fn cli_names_cover_all_job_kinds() {
        // 契约：CLI 子命令（除 serve）== JobKind 全集（现在同源于 JOB_SPECS）。
        let cmd = command();
        let cli: std::collections::BTreeSet<String> = cmd
            .get_subcommands()
            .map(|s| s.get_name().to_string())
            .collect();
        assert_eq!(cli.len(), JOB_SPECS.len() + 1); // + serve
    }
}
