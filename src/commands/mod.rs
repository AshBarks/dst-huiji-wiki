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
//! （后者只写与缺省不同的字段）；`JobKind` 由 spec 表驱动构造，无手写臂。

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
///
/// 绝大多数作业只有 `name`/`about`/`shorts` 与 spec 不同，其余用
/// [`CliExt::DEFAULT`] 补齐：新增条目写成
/// `CliExt { name, about, shorts, ..CliExt::DEFAULT }`。
#[derive(Clone, Copy)]
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

impl CliExt {
    /// 缺省 CLI 视角：无定位参数、无短旗标、无重命名/多值、无跨切面旗标。
    const DEFAULT: CliExt = CliExt {
        name: "",
        about: "",
        positionals: &[],
        shorts: &[],
        renames: NO_RENAMES,
        comma_lists: &[],
        write_args: false,
        report_json: false,
        local_dry_run: false,
    };
}

static CLI_EXT: &[CliExt] = &[
    CliExt {
        name: "parse-po",
        about: "解析 PO 翻译文件为 wiki JSON",
        shorts: &[("input", 'i'), ("output", 'o'), ("category", 'c')],
        ..CliExt::DEFAULT
    },
    CliExt {
        name: "map-names",
        about: "把 PO 物品名映射为 wiki JSON",
        shorts: &[("input", 'i'), ("output", 'o'), ("compare", 'c'), ("merge", 'm'), ("version", 'v')],
        ..CliExt::DEFAULT
    },
    CliExt {
        name: "map-recipes",
        about: "把游戏配方映射为 wiki JSON",
        shorts: &[("input", 'i'), ("output", 'o'), ("compare", 'c'), ("merge", 'm'), ("version", 'v')],
        ..CliExt::DEFAULT
    },
    CliExt {
        name: "maintain-item-table",
        about: "维护 模块:ItemTable（对比线上数据后写维基）",
        shorts: &[("output", 'o')],
        write_args: true,
        report_json: true,
        ..CliExt::DEFAULT
    },
    CliExt {
        name: "maintain-dst-recipes",
        about: "维护 模块:DstRecipes（对比线上数据后写维基）",
        shorts: &[("output", 'o')],
        write_args: true,
        report_json: true,
        ..CliExt::DEFAULT
    },
    CliExt {
        name: "maintain-copy-clip",
        about: "维护模块常量（rbtl/tech/filters/names，标记区间替换）",
        shorts: &[("output", 'o'), ("type", 't')],
        write_args: true,
        report_json: true,
        ..CliExt::DEFAULT
    },
    CliExt {
        name: "maintain-template-check",
        about: "只读检查 模板:Tech/dst 与 模板:制作栏图标 对游戏数据的覆盖率（可选把可粘贴片段写入 --output；不写维基）",
        shorts: &[("output", 'o')],
        report_json: true,
        ..CliExt::DEFAULT
    },
    CliExt {
        name: "maintain-strings",
        about: "解析游戏 strings.pot / chinese_s.po，生成 模块:<V> Strings CN/EN <NN> 桶页与 Data:<V>_Strings_Index.json；逐桶语义对比后只写变化页，索引最后写",
        shorts: &[("output", 'o')],
        write_args: true,
        report_json: true,
        ..CliExt::DEFAULT
    },
    CliExt {
        name: "skilltree-wiki",
        about: "把游戏 skilltree_<char>.lua 提取为 模块:Skilltree/<Char> 子页面的 defs JSON 并维护维基子页面（保留页内 metainfo/icon_url；--output 同时写出 Skilltree.js 渲染器与图片清单）",
        shorts: &[("character", 'c'), ("output", 'o')],
        write_args: true,
        report_json: true,
        ..CliExt::DEFAULT
    },
    CliExt {
        name: "skilltree-export",
        about: "把技能树数据导出为本地 JSON 文件（纯本地，不访问维基）",
        shorts: &[("character", 'c'), ("output", 'o')],
        report_json: true,
        ..CliExt::DEFAULT
    },
    CliExt {
        name: "upload-image",
        about: "上传本地图片到维基（同名重传请加 --ignore-warnings；按 skilltree/ skilltree_icons/ inventoryimages/ 目录自动补分类描述）",
        positionals: &[("path", true)],
        write_args: true,
        report_json: true,
        ..CliExt::DEFAULT
    },
    CliExt {
        name: "prefab-overrides",
        about: "解析单个 Lua 文件的预制体重定向",
        shorts: &[("input", 'i'), ("output", 'o')],
        ..CliExt::DEFAULT
    },
    CliExt {
        name: "prefab-overrides-dir",
        about: "扫描目录下全部 Lua 文件解析预制体重定向并合并（纯本地操作；目录缺省为 DST__ROOT/data/databundles/scripts/prefabs）",
        shorts: &[("input", 'i'), ("output", 'o')],
        ..CliExt::DEFAULT
    },
    CliExt {
        name: "prefab-overrides-audit",
        about: "只读审计 模块:ItemTable/PrefabOverrides：线上条目 × 本地推导做 key/target 存在性验证，输出可推导/需人工补丁/可疑三张清单",
        shorts: &[("output", 'o')],
        ..CliExt::DEFAULT
    },
    CliExt {
        name: "maintain-prefab-overrides",
        about: "维护 模块:ItemTable/PrefabOverrides（只读 diff，不写维基）：本地推导与线上页面合并对比，产出更新/新增清单与本地页面文本",
        shorts: &[("output", 'o')],
        ..CliExt::DEFAULT
    },
    CliExt {
        name: "scripts-sync",
        about: "同步更新后的游戏 scripts:归档旧树为快照、解压新 scripts.zip(纯本地操作)",
        // `--state`（CLI 习惯名）写入 serde 的 `state_path`。
        renames: &[FlagRename { key: "state_path", long: "state", invert: false }],
        report_json: true,
        ..CliExt::DEFAULT
    },
    CliExt {
        name: "images-sync",
        about: "处理游戏图片资源:解压 images.zip、ktech 解码、按 xml 切割(纯本地图片操作),结束后刷新图标元数据并查询维基上传状态(可用 --skip-wiki-status 跳过)",
        report_json: true,
        ..CliExt::DEFAULT
    },
    CliExt {
        name: "upload-icons",
        about: "上传图标到维基（物品栏/制作栏/技能树；标题按来源自动生成或取映射表，描述写对应分类；批量默认仅上传维基缺失的，需先运行 images-sync；--file + --title 手动指定站内文件名）",
        renames: &[FlagRename { key: "only_missing", long: "include-existing", invert: true }],
        write_args: true,
        report_json: true,
        ..CliExt::DEFAULT
    },
    CliExt {
        name: "anim-sync",
        about: "动画历史同步:更新后扫描 data/anim，归档 zip/dyn 到 ANIM__OUT_DIR",
        report_json: true,
        ..CliExt::DEFAULT
    },
    CliExt {
        name: "anim-diff",
        about: "对比两个动画目录的 zip/dyn 结构化 diff",
        positionals: &[("old", true), ("new", true)],
        report_json: true,
        ..CliExt::DEFAULT
    },
    CliExt {
        name: "anim-index",
        about: "构建 prefab 与动画文件的关联索引（纯本地，不做皮肤关联）",
        positionals: &[("scripts", true)],
        shorts: &[("out", 'o')],
        report_json: true,
        ..CliExt::DEFAULT
    },
    CliExt {
        name: "skin-index",
        about: "解析 skinprefabs.lua 生成 base_prefab → skins 皮肤索引（纯本地）",
        shorts: &[("out", 'o')],
        report_json: true,
        ..CliExt::DEFAULT
    },
    CliExt {
        name: "corpus-fetch",
        about: "抓取维基主命名空间全量语料到本地目录（默认 wikis/，不入仓库）；唯一允许匿名读的作业",
        local_dry_run: true,
        ..CliExt::DEFAULT
    },
    CliExt {
        name: "update-scan",
        about: "快照差异 + 关联影响评估（M1，只读）：产出 impact.json 与 changes.patch",
        positionals: &[("old", true), ("new", true)],
        shorts: &[("out", 'o')],
        ..CliExt::DEFAULT
    },
    CliExt {
        name: "update-index",
        about: "构建代码关联索引（基础设施A）并缓存到 output/atlas/<build>/",
        positionals: &[("root", true)],
        shorts: &[("out", 'o')],
        ..CliExt::DEFAULT
    },
    CliExt {
        name: "corpus-index",
        about: "从本地语料树重建派生索引（prefab 注册表等，纯本地操作）",
        local_dry_run: true,
        ..CliExt::DEFAULT
    },
    CliExt {
        name: "symbol-annotate",
        about: "Page→Symbol 标注 CLI：生成高引用 symbol 证据包和 Prompt，可选读取已有 LLM 输出并生成跨页一致性报告（纯本地，不写 wiki）",
        positionals: &[("root", true)],
        ..CliExt::DEFAULT
    },
    CliExt {
        name: "knowledge-scan-symbols",
        about: "M1:LLM 阅读符号源码,产出/更新 SymbolDoc 知识文档",
        positionals: &[("root", true)],
        comma_lists: &["pass2_names", "pick_names"],
        ..CliExt::DEFAULT
    },
    CliExt {
        name: "page-assist",
        about: "page-assist:给定页面输出覆盖缺口建议清单(读 PageSymbolMap,不调 LLM)",
        positionals: &[("page", false)],
        ..CliExt::DEFAULT
    },
    CliExt {
        name: "knowledge-sync",
        about: "M3:代码变更 → 脏 SymbolDoc → 页面锚点交叉(确定性;--rescan 级联重扫)",
        // `old` 在 --review 复核模式下可省略；`new` 有默认值 "current"。
        positionals: &[("old", false), ("new", false)],
        ..CliExt::DEFAULT
    },
    CliExt {
        name: "knowledge-scan-wiki",
        about: "M2a:PageSymbolMap 确定性骨架(路由/反转/数值配对,不调 LLM)",
        positionals: &[("root", true)],
        comma_lists: &["audit_symbols"],
        ..CliExt::DEFAULT
    },
    CliExt {
        name: "maintain-wikitext",
        about: "用 wikitext 解析器批量修改页面里指定模板的参数（外科手术式编辑，未修改部分逐字节还原；--dry-run 只产 diff 报告不写维基）",
        shorts: &[("output", 'o')],
        renames: &[FlagRename { key: "pages", long: "page", invert: false }],
        comma_lists: &["pages", "set", "remove"],
        write_args: true,
        report_json: true,
        ..CliExt::DEFAULT
    },
    CliExt {
        name: "create-redirect",
        about: "在 wiki 上创建页面重定向（#REDIRECT [[目标]]；--dry-run 只预览 wikitext）",
        positionals: &[("from", true), ("to", true)],
        write_args: true,
        report_json: true,
        ..CliExt::DEFAULT
    },
    CliExt {
        name: "cooking-game-export",
        about: "把料理模拟数据、图标与 example_combo 打包给独立烹饪小游戏项目",
        shorts: &[("output", 'o')],
        report_json: true,
        ..CliExt::DEFAULT
    },
    CliExt {
        name: "update-local",
        about: "更新后本地摄取编排：scripts-sync → images-sync → anim-sync（缺省串行，任一步失败即中止）",
        report_json: true,
        ..CliExt::DEFAULT
    },
    CliExt {
        name: "update-data",
        about: "更新后数据页编排：maintain-copy-clip → maintain-dst-recipes → maintain-item-table → maintain-strings（共用 --snapshot 与写入模式）",
        write_args: true,
        report_json: true,
        ..CliExt::DEFAULT
    },
    CliExt {
        name: "update-assets",
        about: "更新后素材编排：upload-icons → skilltree-wiki（任一步失败即中止）",
        write_args: true,
        report_json: true,
        ..CliExt::DEFAULT
    },
    CliExt {
        name: "update-check",
        about: "更新后只读核对：maintain-template-check + prefab-overrides-audit（continue-on-error）",
        report_json: true,
        ..CliExt::DEFAULT
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

/// 构建完整 CLI（顶层 + serve + 38 个 job 子命令）。
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
        ParamKind::StrList | ParamKind::OptStrList => {
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

fn num(m: &clap::ArgMatches, key: &str) -> usize {
    m.get_one::<String>(key)
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| panic!("参数 {key} 不是非负整数"))
}

fn list(m: &clap::ArgMatches, key: &str) -> Vec<String> {
    m.get_many::<String>(key)
        .map(|v| v.cloned().collect())
        .unwrap_or_default()
}

/// 按 spec/CLI 元数据把 `ArgMatches` 组装成 [`JobKind`]。
///
/// 不再逐作业手写构造臂：按 [`ParamKind`] 从匹配结果取值（重命名用长旗标、
/// `invert` 的 Bool 取反），拼成 JSON 后交给 serde。这样 CLI 参数、Web JSON
/// 与 spec 表三者的对齐是**机器保证**的——`contract_every_spec_constructs_*`
/// 会逐个作业验证 spec 声明的 kind 与变体字段类型一致。
fn job_from_matches(
    spec: &dst_huiji_wiki::service::JobSpec,
    ext: &CliExt,
    m: &clap::ArgMatches,
) -> Result<JobKind> {
    use serde_json::{json, Map, Value};

    let mut obj = Map::new();
    obj.insert("kind".into(), json!(spec.name.replace('-', "_")));

    for param in spec.params {
        let invert = ext.renames.iter().any(|r| r.key == param.key && r.invert);
        let value: Value = if invert {
            // 取反旗标：CLI 给 true 表示参数为 false（只取 spec 默认的反面）。
            json!(!m.get_flag(param.key))
        } else {
            match param.kind {
                ParamKind::Bool => json!(m.get_flag(param.key)),
                ParamKind::Usize => json!(num(m, param.key)),
                ParamKind::StrList => json!(list(m, param.key)),
                ParamKind::OptStrList => match list(m, param.key) {
                    v if v.is_empty() => Value::Null,
                    v => json!(v),
                },
                ParamKind::OptStr | ParamKind::OptPath => match opt_val(m, param.key) {
                    Some(v) => json!(v),
                    None => Value::Null,
                },
                ParamKind::Str | ParamKind::Path => json!(val(m, param.key)),
            }
        };
        obj.insert(param.key.into(), value);
    }

    serde_json::from_value(Value::Object(obj)).map_err(|e| {
        dst_huiji_wiki::Error::Config(format!(
            "作业 `{}` 的 CLI 参数无法映射到 JobKind（spec 的 ParamKind 与 serde 字段类型不一致？）: {e}",
            spec.name
        ))
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

    /// 契约：每个 spec 都能用 CLI 默认值（+ 定位参数占位）构造出 JobKind，
    /// 且 name 与 spec 一致。这是"表驱动构造"的兜底：spec 的 `ParamKind`
    /// 与变体字段类型不一致时，这里会直接失败。
    #[test]
    fn contract_every_spec_constructs_from_cli_defaults() {
        for spec in JOB_SPECS {
            let ext = ext(spec.name);
            let mut argv = vec!["dst-huiji-wiki".to_string(), spec.name.to_string()];
            // 定位参数按声明顺序各给一个占位值（必填/可选都接受）。
            argv.extend(ext.positionals.iter().map(|_| "placeholder".to_string()));
            // 必填的**非定位**参数必须显式给值；Usize 不能是占位词。
            for p in spec.params.iter().filter(|p| {
                p.required
                    && !ext.positionals.iter().any(|(k, _)| *k == p.key)
                    && !ext.renames.iter().any(|r| r.key == p.key)
            }) {
                argv.push(format!("--{}", p.key));
                match p.kind {
                    ParamKind::Bool => {}
                    ParamKind::Usize => argv.push("1".to_string()),
                    _ => argv.push("placeholder".to_string()),
                }
            }

            let matches = command()
                .try_get_matches_from(&argv)
                .unwrap_or_else(|e| panic!("`{}` 参数解析失败: {e}", spec.name));
            let sub = matches
                .subcommand_matches(spec.name)
                .unwrap_or_else(|| panic!("`{}` 子命令缺失", spec.name));
            let kind = job_from_matches(spec, ext, sub)
                .unwrap_or_else(|e| panic!("`{}` 构造失败: {e}", spec.name));
            assert_eq!(kind.name(), spec.name);
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
