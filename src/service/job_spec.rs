//! JobSpec 注册表：每个 Job 的 canonical name、wiki 访问级别、资源竞争域
//! 与参数 schema 的集中声明。
//!
//! 此前 CLI（clap derive）、`JobKind::name()`、serde tag、前端 `JOB_DEFS`
//! 四处各自维护名称与默认值，靠契约测试事后兜底；参数则只有字符串语义
//! 校验，非法 `type`/`source`/路径要到执行中才报错。本表是唯一的声明源：
//!
//! - `JobKind::name()/touches_wiki()/resources()/spec()` 从本表派生；
//! - CLI 子命令与参数（P1.6 起由 `commands` 从本表构建）、Web 前端
//!   `JOB_DEFS` 的键/默认值以契约测试对齐本表；
//! - 资源竞争域供 WebUI JobManager 做跨任务互斥（同名即互斥的键）。

/// Job 对 wiki 的访问级别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WikiAccess {
    /// 不访问维基。
    None,
    /// 只读访问（仍要求登录；corpus-fetch 例外匿名，见其 spec 注释）。
    Read,
    /// 可能写维基（受 WriteMode/确认流程约束）。
    Write,
}

/// 参数值类型（serde/JSON 视角；CLI 侧 PathBuf 与 String 等价转换）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamKind {
    Str,
    Path,
    OptStr,
    OptPath,
    Bool,
    Usize,
    StrList,
}

/// 参数默认值（Web 前端展示与校验共用）。
#[derive(Debug, Clone, Copy)]
pub enum ParamDefault {
    None,
    Str(&'static str),
    Int(usize),
    Bool(bool),
}

/// 一个参数的声明。
#[derive(Debug, Clone, Copy)]
pub struct ParamSpec {
    /// serde 字段名（= Web JSON 参数键）。
    pub key: &'static str,
    pub kind: ParamKind,
    pub default: ParamDefault,
    /// 缺省即报错（仅必填的定位参数）。
    pub required: bool,
    /// 帮助文本（CLI --help 与 Web 前端共用）。
    pub help: &'static str,
}

/// 一个 Job 的声明。
#[derive(Debug, Clone, Copy)]
pub struct JobSpec {
    /// canonical 名称：CLI 子命令名 == `JobKind::name()`；serde tag 为其
    /// snake 形式（契约测试强制）。
    pub name: &'static str,
    /// 中文显示名（Web 前端 / 日志）。
    pub label: &'static str,
    pub wiki_access: WikiAccess,
    /// 资源竞争域：WebUI JobManager 对同名资源全程互斥（如两个
    /// images-sync 并发写同一 KTOOLS__OUT_DIR）。写维基的任务一律含
    /// "wiki"。
    pub resources: &'static [&'static str],
    pub params: &'static [ParamSpec],
}

const P_INPUT: ParamSpec = ParamSpec {
    key: "input",
    kind: ParamKind::Path,
    default: ParamDefault::None,
    required: true,
    help: "输入文件路径",
};
const P_OUTPUT: ParamSpec = ParamSpec {
    key: "output",
    kind: ParamKind::OptPath,
    default: ParamDefault::None,
    required: false,
    help: "输出路径（可选）",
};

/// 全部 Job 的声明表（顺序与 `JobKind::all_variants()` 一致）。
pub static JOB_SPECS: &[JobSpec] = &[
    JobSpec {
        name: "parse-po",
        label: "解析 PO 文件",
        wiki_access: WikiAccess::None,
        resources: &["out"],
        params: &[
            P_INPUT,
            P_OUTPUT,
            ParamSpec {
                key: "category",
                kind: ParamKind::OptStr,
                default: ParamDefault::None,
                required: false,
                help: "按 PO 分类过滤（可选）",
            },
        ],
    },
    JobSpec {
        name: "map-names",
        label: "映射物品名称",
        wiki_access: WikiAccess::None,
        resources: &["out"],
        params: &[
            P_INPUT,
            P_OUTPUT,
            ParamSpec {
                key: "compare",
                kind: ParamKind::OptPath,
                default: ParamDefault::None,
                required: false,
                help: "与既有 wiki JSON 对比（可选）",
            },
            ParamSpec {
                key: "merge",
                kind: ParamKind::Bool,
                default: ParamDefault::Bool(false),
                required: false,
                help: "合并而非覆盖",
            },
            ParamSpec {
                key: "version",
                kind: ParamKind::OptStr,
                default: ParamDefault::None,
                required: false,
                help: "版本标注（可选）",
            },
        ],
    },
    JobSpec {
        name: "map-recipes",
        label: "映射配方数据",
        wiki_access: WikiAccess::None,
        resources: &["out"],
        params: &[
            P_INPUT,
            P_OUTPUT,
            ParamSpec {
                key: "compare",
                kind: ParamKind::OptPath,
                default: ParamDefault::None,
                required: false,
                help: "与既有 wiki JSON 对比（可选）",
            },
            ParamSpec {
                key: "merge",
                kind: ParamKind::Bool,
                default: ParamDefault::Bool(false),
                required: false,
                help: "合并而非覆盖",
            },
            ParamSpec {
                key: "po_file",
                kind: ParamKind::OptPath,
                default: ParamDefault::None,
                required: false,
                help: "PO 文件路径（可选）",
            },
            ParamSpec {
                key: "version",
                kind: ParamKind::OptStr,
                default: ParamDefault::None,
                required: false,
                help: "版本标注（可选）",
            },
        ],
    },
    JobSpec {
        name: "maintain-item-table",
        label: "维护物品表 → 维基",
        wiki_access: WikiAccess::Write,
        resources: &["wiki", "out"],
        params: &[
            P_OUTPUT,
            ParamSpec {
                key: "snapshot",
                kind: ParamKind::OptStr,
                default: ParamDefault::None,
                required: false,
                help: "使用指定 scripts 快照",
            },
        ],
    },
    JobSpec {
        name: "maintain-dst-recipes",
        label: "维护配方表 → 维基",
        wiki_access: WikiAccess::Write,
        resources: &["wiki", "out"],
        params: &[
            P_OUTPUT,
            ParamSpec {
                key: "snapshot",
                kind: ParamKind::OptStr,
                default: ParamDefault::None,
                required: false,
                help: "使用指定 scripts 快照",
            },
        ],
    },
    JobSpec {
        name: "maintain-copy-clip",
        label: "维护模块常量 → 维基",
        wiki_access: WikiAccess::Write,
        resources: &["wiki", "out"],
        params: &[
            ParamSpec {
                key: "type",
                kind: ParamKind::OptStr,
                default: ParamDefault::None,
                required: false,
                help: "类型：rbtl / tech / filters / names（留空=全部）",
            },
            P_OUTPUT,
            ParamSpec {
                key: "snapshot",
                kind: ParamKind::OptStr,
                default: ParamDefault::None,
                required: false,
                help: "使用指定 scripts 快照",
            },
        ],
    },
    JobSpec {
        name: "maintain-template-check",
        label: "检查模板数据覆盖（只读）",
        wiki_access: WikiAccess::Read,
        resources: &[],
        params: &[
            ParamSpec {
                key: "output",
                kind: ParamKind::OptPath,
                default: ParamDefault::None,
                required: false,
                help: "可粘贴片段输出文件（可选）",
            },
            ParamSpec {
                key: "snapshot",
                kind: ParamKind::OptStr,
                default: ParamDefault::None,
                required: false,
                help: "使用指定 scripts 快照",
            },
            ParamSpec {
                key: "skip_icon_status",
                kind: ParamKind::Bool,
                default: ParamDefault::Bool(false),
                required: false,
                help: "跳过 live 图标存在性查询",
            },
        ],
    },
    JobSpec {
        name: "maintain-strings",
        label: "维护 Strings 桶页 → 维基",
        wiki_access: WikiAccess::Write,
        resources: &["wiki", "out"],
        params: &[
            ParamSpec {
                key: "version",
                kind: ParamKind::Str,
                default: ParamDefault::Str("DST"),
                required: false,
                help: "版本前缀（页面名与索引名）",
            },
            ParamSpec {
                key: "snapshot",
                kind: ParamKind::OptStr,
                default: ParamDefault::None,
                required: false,
                help: "使用指定 scripts 快照",
            },
            ParamSpec {
                key: "output",
                kind: ParamKind::OptPath,
                default: ParamDefault::None,
                required: false,
                help: "本地产物输出目录",
            },
            ParamSpec {
                key: "offline",
                kind: ParamKind::Bool,
                default: ParamDefault::Bool(false),
                required: false,
                help: "跳过现网对比（无维基流量）",
            },
            ParamSpec {
                key: "bucket_count",
                kind: ParamKind::Usize,
                default: ParamDefault::Int(100),
                required: false,
                help: "无现有索引时的等分桶数",
            },
            ParamSpec {
                key: "rebalance",
                kind: ParamKind::Bool,
                default: ParamDefault::Bool(false),
                required: false,
                help: "忽略现有索引边界，强制等量重切",
            },
            ParamSpec {
                key: "limit",
                kind: ParamKind::Usize,
                default: ParamDefault::Int(0),
                required: false,
                help: "Canary：最多写入 N 个桶页（0=不限制）",
            },
        ],
    },
    JobSpec {
        name: "skilltree-wiki",
        label: "维护技能树子页面 → 维基",
        wiki_access: WikiAccess::Write,
        resources: &["wiki", "out"],
        params: &[
            ParamSpec {
                key: "character",
                kind: ParamKind::OptStr,
                default: ParamDefault::None,
                required: false,
                help: "角色过滤子串（如 walter）",
            },
            P_OUTPUT,
            ParamSpec {
                key: "snapshot",
                kind: ParamKind::OptStr,
                default: ParamDefault::None,
                required: false,
                help: "使用指定 scripts 快照",
            },
        ],
    },
    JobSpec {
        name: "skilltree-export",
        label: "技能树数据导出到本地",
        wiki_access: WikiAccess::None,
        resources: &["out"],
        params: &[
            ParamSpec {
                key: "character",
                kind: ParamKind::OptStr,
                default: ParamDefault::None,
                required: false,
                help: "角色过滤子串（如 walter）",
            },
            ParamSpec {
                key: "output",
                kind: ParamKind::OptPath,
                default: ParamDefault::None,
                required: false,
                help: "输出目录（默认 output/skilltree）",
            },
            ParamSpec {
                key: "snapshot",
                kind: ParamKind::OptStr,
                default: ParamDefault::None,
                required: false,
                help: "使用指定 scripts 快照",
            },
        ],
    },
    JobSpec {
        name: "upload-image",
        label: "上传图片 → 维基",
        wiki_access: WikiAccess::Write,
        resources: &["wiki"],
        params: &[
            ParamSpec {
                key: "path",
                kind: ParamKind::Path,
                default: ParamDefault::None,
                required: true,
                help: "本地图片路径",
            },
            ParamSpec {
                key: "name",
                kind: ParamKind::OptStr,
                default: ParamDefault::None,
                required: false,
                help: "维基文件名（默认取路径文件名）",
            },
            ParamSpec {
                key: "description",
                kind: ParamKind::OptStr,
                default: ParamDefault::None,
                required: false,
                help: "文件描述 wikitext",
            },
            ParamSpec {
                key: "comment",
                kind: ParamKind::OptStr,
                default: ParamDefault::None,
                required: false,
                help: "上传注释",
            },
            ParamSpec {
                key: "ignore_warnings",
                kind: ParamKind::Bool,
                default: ParamDefault::Bool(false),
                required: false,
                help: "忽略上传警告（同名重传）",
            },
        ],
    },
    JobSpec {
        name: "prefab-overrides",
        label: "预制体重定向解析（单文件）",
        wiki_access: WikiAccess::None,
        resources: &["out"],
        params: &[P_INPUT, P_OUTPUT],
    },
    JobSpec {
        name: "prefab-overrides-dir",
        label: "预制体重定向解析（目录）",
        wiki_access: WikiAccess::None,
        resources: &["out"],
        params: &[
            ParamSpec {
                key: "input",
                kind: ParamKind::OptPath,
                default: ParamDefault::None,
                required: false,
                help: "Lua 目录路径（缺省 DST__ROOT/data/databundles/scripts/prefabs）",
            },
            P_OUTPUT,
        ],
    },
    JobSpec {
        name: "prefab-overrides-audit",
        label: "预制体重定向审计（只读）",
        wiki_access: WikiAccess::Read,
        resources: &[],
        params: &[
            ParamSpec {
                key: "scripts",
                kind: ParamKind::OptPath,
                default: ParamDefault::None,
                required: false,
                help: "游戏脚本根目录（缺省 DST__ROOT/data/databundles/scripts）",
            },
            ParamSpec {
                key: "wiki_file",
                kind: ParamKind::OptPath,
                default: ParamDefault::None,
                required: false,
                help: "线上页面原文文件（缺省在线拉取）",
            },
            ParamSpec {
                key: "output",
                kind: ParamKind::OptPath,
                default: ParamDefault::None,
                required: false,
                help: "审计报告 JSON 输出（可选）",
            },
        ],
    },
    JobSpec {
        name: "maintain-prefab-overrides",
        label: "维护预制体重定向（只读 diff）",
        wiki_access: WikiAccess::Read,
        resources: &["out"],
        params: &[
            ParamSpec {
                key: "scripts",
                kind: ParamKind::OptPath,
                default: ParamDefault::None,
                required: false,
                help: "游戏脚本根目录（缺省 DST__ROOT/data/databundles/scripts）",
            },
            ParamSpec {
                key: "wiki_file",
                kind: ParamKind::OptPath,
                default: ParamDefault::None,
                required: false,
                help: "线上页面原文文件（缺省在线拉取）",
            },
            ParamSpec {
                key: "output",
                kind: ParamKind::OptPath,
                default: ParamDefault::None,
                required: false,
                help: "产物目录：PrefabOverrides.lua + diff.json",
            },
        ],
    },
    JobSpec {
        name: "scripts-sync",
        label: "同步游戏脚本（scripts-sync）",
        wiki_access: WikiAccess::None,
        resources: &["scripts_tree"],
        params: &[
            ParamSpec {
                key: "force",
                kind: ParamKind::Bool,
                default: ParamDefault::Bool(false),
                required: false,
                help: "版本相同也强制同步",
            },
            ParamSpec {
                key: "dry_run",
                kind: ParamKind::Bool,
                default: ParamDefault::Bool(false),
                required: false,
                help: "演练：只报告计划、不动任何文件",
            },
            ParamSpec {
                key: "state_path",
                kind: ParamKind::OptPath,
                default: ParamDefault::None,
                required: false,
                help: "版本状态文件（默认 ./dst_version.txt）",
            },
        ],
    },
    JobSpec {
        name: "images-sync",
        label: "处理游戏图片（images-sync）",
        wiki_access: WikiAccess::None,
        resources: &["ktools_out"],
        params: &[
            ParamSpec {
                key: "force",
                kind: ParamKind::Bool,
                default: ParamDefault::Bool(false),
                required: false,
                help: "忽略增量与幂等检查，全量重跑",
            },
            ParamSpec {
                key: "dry_run",
                kind: ParamKind::Bool,
                default: ParamDefault::Bool(false),
                required: false,
                help: "演练：只盘点并报告计划",
            },
            ParamSpec {
                key: "skip_wiki_status",
                kind: ParamKind::Bool,
                default: ParamDefault::Bool(false),
                required: false,
                help: "跳过维基上传状态查询",
            },
        ],
    },
    JobSpec {
        name: "upload-icons",
        label: "上传图标 → 维基",
        wiki_access: WikiAccess::Write,
        resources: &["wiki"],
        params: &[
            ParamSpec {
                key: "build",
                kind: ParamKind::OptStr,
                default: ParamDefault::None,
                required: false,
                help: "只上传首次加入该 build 的图标",
            },
            ParamSpec {
                key: "source",
                kind: ParamKind::OptStr,
                default: ParamDefault::None,
                required: false,
                help: "来源：inventory / crafting / skilltree（留空=全部）",
            },
            ParamSpec {
                key: "file",
                kind: ParamKind::OptStr,
                default: ParamDefault::None,
                required: false,
                help: "只上传单个文件名（如 axe.png）",
            },
            ParamSpec {
                key: "title",
                kind: ParamKind::OptStr,
                default: ParamDefault::None,
                required: false,
                help: "手动指定 wiki 文件名（需配合 file）",
            },
            ParamSpec {
                key: "only_missing",
                kind: ParamKind::Bool,
                default: ParamDefault::Bool(true),
                required: false,
                help: "批量仅上传维基缺失的图标",
            },
            ParamSpec {
                key: "ignore_warnings",
                kind: ParamKind::Bool,
                default: ParamDefault::Bool(false),
                required: false,
                help: "同名重传忽略警告",
            },
            ParamSpec {
                key: "comment",
                kind: ParamKind::OptStr,
                default: ParamDefault::None,
                required: false,
                help: "上传注释（可选）",
            },
        ],
    },
    JobSpec {
        name: "anim-sync",
        label: "动画历史同步（anim-sync）",
        wiki_access: WikiAccess::None,
        resources: &["anim_out"],
        params: &[
            ParamSpec {
                key: "force",
                kind: ParamKind::Bool,
                default: ParamDefault::Bool(false),
                required: false,
                help: "强制重新扫描/归档",
            },
            ParamSpec {
                key: "dry_run",
                kind: ParamKind::Bool,
                default: ParamDefault::Bool(false),
                required: false,
                help: "演练：只盘点并报告计划",
            },
            ParamSpec {
                key: "label",
                kind: ParamKind::OptStr,
                default: ParamDefault::None,
                required: false,
                help: "版本标签（默认 version.txt）",
            },
            ParamSpec {
                key: "out",
                kind: ParamKind::OptPath,
                default: ParamDefault::None,
                required: false,
                help: "历史根目录（默认 $ANIM__OUT_DIR）",
            },
        ],
    },
    JobSpec {
        name: "anim-diff",
        label: "动画目录对比",
        wiki_access: WikiAccess::None,
        resources: &[],
        params: &[
            ParamSpec {
                key: "old",
                kind: ParamKind::Path,
                default: ParamDefault::None,
                required: true,
                help: "旧动画目录",
            },
            ParamSpec {
                key: "new",
                kind: ParamKind::Path,
                default: ParamDefault::None,
                required: true,
                help: "新动画目录",
            },
            ParamSpec {
                key: "zip",
                kind: ParamKind::OptStr,
                default: ParamDefault::None,
                required: false,
                help: "只对比某个相对路径",
            },
        ],
    },
    JobSpec {
        name: "anim-index",
        label: "构建 prefab↔动画关联索引",
        wiki_access: WikiAccess::None,
        resources: &["anim_out"],
        params: &[
            ParamSpec {
                key: "scripts",
                kind: ParamKind::Path,
                default: ParamDefault::None,
                required: true,
                help: "游戏脚本根目录",
            },
            ParamSpec {
                key: "anim",
                kind: ParamKind::OptPath,
                default: ParamDefault::None,
                required: false,
                help: "动画资源目录（缺省由 scripts 推导）",
            },
            ParamSpec {
                key: "out",
                kind: ParamKind::OptPath,
                default: ParamDefault::None,
                required: false,
                help: "输出目录（可选）",
            },
        ],
    },
    JobSpec {
        name: "skin-index",
        label: "生成皮肤动画索引",
        wiki_access: WikiAccess::None,
        resources: &["out"],
        params: &[
            ParamSpec {
                key: "scripts",
                kind: ParamKind::OptPath,
                default: ParamDefault::None,
                required: false,
                help: "游戏脚本根目录（缺省 DST__ROOT/data/databundles/scripts）",
            },
            ParamSpec {
                key: "anim",
                kind: ParamKind::OptPath,
                default: ParamDefault::None,
                required: false,
                help: "动画资源目录（可选）",
            },
            ParamSpec {
                key: "out",
                kind: ParamKind::OptPath,
                default: ParamDefault::None,
                required: false,
                help: "输出 JSON（默认 output/skin-index.json）",
            },
        ],
    },
    JobSpec {
        name: "update-scan",
        label: "快照 diff + 影响评估",
        wiki_access: WikiAccess::None,
        resources: &["out"],
        params: &[
            ParamSpec {
                key: "old",
                kind: ParamKind::Path,
                default: ParamDefault::None,
                required: true,
                help: "旧快照（时间戳或目录名）",
            },
            ParamSpec {
                key: "new",
                kind: ParamKind::Path,
                default: ParamDefault::None,
                required: true,
                help: "新快照（时间戳、目录名或 current）",
            },
            ParamSpec {
                key: "out",
                kind: ParamKind::OptPath,
                default: ParamDefault::None,
                required: false,
                help: "输出目录（可选）",
            },
            ParamSpec {
                key: "corpus",
                kind: ParamKind::OptPath,
                default: ParamDefault::None,
                required: false,
                help: "语料根目录（提供时附加 Layer B 摘要）",
            },
            ParamSpec {
                key: "annotate",
                kind: ParamKind::OptPath,
                default: ParamDefault::None,
                required: false,
                help: "输出 fn 标注骨架到该目录",
            },
        ],
    },
    JobSpec {
        name: "update-index",
        label: "构建代码关联图",
        wiki_access: WikiAccess::None,
        resources: &["out"],
        params: &[
            ParamSpec {
                key: "root",
                kind: ParamKind::Path,
                default: ParamDefault::None,
                required: true,
                help: "游戏脚本根目录",
            },
            ParamSpec {
                key: "out",
                kind: ParamKind::OptPath,
                default: ParamDefault::None,
                required: false,
                help: "输出目录（可选）",
            },
        ],
    },
    JobSpec {
        name: "corpus-fetch",
        label: "抓取维基语料（corpus-fetch）",
        // 唯一允许匿名读的作业：批量全量抓取可接受站点旧缓存。
        wiki_access: WikiAccess::Read,
        resources: &["corpus"],
        params: &[
            ParamSpec {
                key: "full",
                kind: ParamKind::Bool,
                default: ParamDefault::Bool(false),
                required: false,
                help: "忽略 touched 增量，全量抓取",
            },
            ParamSpec {
                key: "dir",
                kind: ParamKind::OptPath,
                default: ParamDefault::None,
                required: false,
                help: "语料根目录（默认 wikis/）",
            },
            ParamSpec {
                key: "rc",
                kind: ParamKind::Bool,
                default: ParamDefault::Bool(false),
                required: false,
                help: "recentchanges 增量通道",
            },
        ],
    },
    JobSpec {
        name: "corpus-index",
        label: "重建语料派生索引",
        wiki_access: WikiAccess::None,
        resources: &["corpus"],
        params: &[
            ParamSpec {
                key: "dir",
                kind: ParamKind::OptPath,
                default: ParamDefault::None,
                required: false,
                help: "语料根目录（默认 wikis/）",
            },
            ParamSpec {
                key: "join",
                kind: ParamKind::OptPath,
                default: ParamDefault::None,
                required: false,
                help: "代码侧 index.json（可选，校准报告）",
            },
        ],
    },
    JobSpec {
        name: "symbol-annotate",
        label: "Page→Symbol 标注",
        wiki_access: WikiAccess::None,
        resources: &["out"],
        params: &[
            ParamSpec {
                key: "root",
                kind: ParamKind::Path,
                default: ParamDefault::None,
                required: true,
                help: "游戏脚本根目录",
            },
            ParamSpec {
                key: "corpus",
                kind: ParamKind::Path,
                default: ParamDefault::None,
                required: true,
                help: "语料根目录",
            },
            ParamSpec {
                key: "limit",
                kind: ParamKind::Usize,
                default: ParamDefault::Int(20),
                required: false,
                help: "最多标注的符号数",
            },
            ParamSpec {
                key: "out",
                kind: ParamKind::OptPath,
                default: ParamDefault::None,
                required: false,
                help: "输出目录（可选）",
            },
            ParamSpec {
                key: "verdicts",
                kind: ParamKind::OptPath,
                default: ParamDefault::None,
                required: false,
                help: "LLM 判定文件（可选）",
            },
            ParamSpec {
                key: "llm",
                kind: ParamKind::Bool,
                default: ParamDefault::Bool(false),
                required: false,
                help: "调用 LLM 生成判定",
            },
            ParamSpec {
                key: "batch_pages",
                kind: ParamKind::Usize,
                default: ParamDefault::Int(40),
                required: false,
                help: "每批最多页数",
            },
            ParamSpec {
                key: "batch_max_chars",
                kind: ParamKind::Usize,
                default: ParamDefault::Int(32_000),
                required: false,
                help: "每批字节预算",
            },
            ParamSpec {
                key: "skip_no_fact_pages",
                kind: ParamKind::Bool,
                default: ParamDefault::Bool(false),
                required: false,
                help: "零候选证据页不送 LLM",
            },
        ],
    },
    JobSpec {
        name: "knowledge-scan-symbols",
        label: "LLM 扫描符号生成知识文档",
        wiki_access: WikiAccess::None,
        resources: &["knowledge"],
        params: &[
            ParamSpec {
                key: "root",
                kind: ParamKind::Path,
                default: ParamDefault::None,
                required: true,
                help: "游戏脚本根目录",
            },
            ParamSpec {
                key: "category",
                kind: ParamKind::Str,
                default: ParamDefault::Str("component"),
                required: false,
                help: "符号类别",
            },
            ParamSpec {
                key: "knowledge_dir",
                kind: ParamKind::Str,
                default: ParamDefault::Str("knowledge"),
                required: false,
                help: "知识库目录",
            },
            ParamSpec {
                key: "corpus",
                kind: ParamKind::OptPath,
                default: ParamDefault::None,
                required: false,
                help: "语料根目录（启用 pass2 归因）",
            },
            ParamSpec {
                key: "sample_pages",
                kind: ParamKind::Usize,
                default: ParamDefault::Int(8),
                required: false,
                help: "每符号采样页数",
            },
            ParamSpec {
                key: "limit",
                kind: ParamKind::Usize,
                default: ParamDefault::Int(20),
                required: false,
                help: "最多处理的符号数",
            },
            ParamSpec {
                key: "force",
                kind: ParamKind::Bool,
                default: ParamDefault::Bool(false),
                required: false,
                help: "忽略既有文档强制重扫",
            },
            ParamSpec {
                key: "concurrency",
                kind: ParamKind::Usize,
                default: ParamDefault::Int(1),
                required: false,
                help: "并行组件数（1=串行）",
            },
            ParamSpec {
                key: "pass2_names",
                kind: ParamKind::StrList,
                default: ParamDefault::None,
                required: false,
                help: "仅 pass2 指定文件名词干",
            },
            ParamSpec {
                key: "pick_names",
                kind: ParamKind::StrList,
                default: ParamDefault::None,
                required: false,
                help: "仅处理指定符号",
            },
            ParamSpec {
                key: "refresh_auto",
                kind: ParamKind::Bool,
                default: ParamDefault::Bool(false),
                required: false,
                help: "仅刷新现有文档 auto_maintained",
            },
            ParamSpec {
                key: "confirm_empty",
                kind: ParamKind::Bool,
                default: ParamDefault::Bool(false),
                required: false,
                help: "pass2 二次确认",
            },
        ],
    },
    JobSpec {
        name: "page-assist",
        label: "页面覆盖缺口建议（只读）",
        wiki_access: WikiAccess::None,
        resources: &[],
        params: &[
            ParamSpec {
                key: "knowledge_dir",
                kind: ParamKind::Str,
                default: ParamDefault::Str("knowledge"),
                required: false,
                help: "知识库目录",
            },
            ParamSpec {
                key: "page",
                kind: ParamKind::OptStr,
                default: ParamDefault::None,
                required: false,
                help: "目标页面（可选）",
            },
            ParamSpec {
                key: "all",
                kind: ParamKind::Bool,
                default: ParamDefault::Bool(false),
                required: false,
                help: "输出全部页面的缺口",
            },
            ParamSpec {
                key: "json",
                kind: ParamKind::Bool,
                default: ParamDefault::Bool(false),
                required: false,
                help: "JSON 输出",
            },
            ParamSpec {
                key: "attribute",
                kind: ParamKind::Bool,
                default: ParamDefault::Bool(false),
                required: false,
                help: "编辑归因模式（需 corpus）",
            },
            ParamSpec {
                key: "corpus",
                kind: ParamKind::OptPath,
                default: ParamDefault::None,
                required: false,
                help: "语料根目录（可选）",
            },
        ],
    },
    JobSpec {
        name: "knowledge-sync",
        label: "代码变更 → 知识文档同步",
        wiki_access: WikiAccess::None,
        resources: &["knowledge"],
        params: &[
            ParamSpec {
                key: "old",
                kind: ParamKind::Str,
                default: ParamDefault::None,
                required: true,
                help: "旧快照（时间戳或目录名）",
            },
            ParamSpec {
                key: "new",
                kind: ParamKind::Str,
                default: ParamDefault::Str("current"),
                required: false,
                help: "新快照（默认 current）",
            },
            ParamSpec {
                key: "knowledge_dir",
                kind: ParamKind::Str,
                default: ParamDefault::Str("knowledge"),
                required: false,
                help: "知识库目录",
            },
            ParamSpec {
                key: "rescan",
                kind: ParamKind::Bool,
                default: ParamDefault::Bool(false),
                required: false,
                help: "级联 LLM 重扫",
            },
            ParamSpec {
                key: "limit",
                kind: ParamKind::Usize,
                default: ParamDefault::Int(20),
                required: false,
                help: "最多处理的脏文档数",
            },
            ParamSpec {
                key: "corpus",
                kind: ParamKind::OptPath,
                default: ParamDefault::None,
                required: false,
                help: "语料根目录（启用页面交叉）",
            },
            ParamSpec {
                key: "draft",
                kind: ParamKind::Bool,
                default: ParamDefault::Bool(false),
                required: false,
                help: "起草页面修订建议（Tier2）",
            },
            ParamSpec {
                key: "review",
                kind: ParamKind::OptPath,
                default: ParamDefault::None,
                required: false,
                help: "裁决文件（approve/reject）",
            },
        ],
    },
    JobSpec {
        name: "knowledge-scan-wiki",
        label: "PageSymbolMap 确定性骨架",
        wiki_access: WikiAccess::None,
        resources: &["knowledge"],
        params: &[
            ParamSpec {
                key: "root",
                kind: ParamKind::Path,
                default: ParamDefault::None,
                required: true,
                help: "游戏脚本根目录",
            },
            ParamSpec {
                key: "knowledge_dir",
                kind: ParamKind::Str,
                default: ParamDefault::Str("knowledge"),
                required: false,
                help: "知识库目录",
            },
            ParamSpec {
                key: "corpus",
                kind: ParamKind::Path,
                default: ParamDefault::None,
                required: true,
                help: "语料根目录",
            },
            ParamSpec {
                key: "audit",
                kind: ParamKind::Bool,
                default: ParamDefault::Bool(false),
                required: false,
                help: "输出跨页一致性审计",
            },
            ParamSpec {
                key: "audit_symbols",
                kind: ParamKind::StrList,
                default: ParamDefault::None,
                required: false,
                help: "审计指定符号",
            },
            ParamSpec {
                key: "audit_max_pages",
                kind: ParamKind::Usize,
                default: ParamDefault::Int(60),
                required: false,
                help: "审计最多页数",
            },
            ParamSpec {
                key: "audit_batch_pages",
                kind: ParamKind::Usize,
                default: ParamDefault::Int(20),
                required: false,
                help: "审计每批页数",
            },
            ParamSpec {
                key: "audit_batch_max_chars",
                kind: ParamKind::Usize,
                default: ParamDefault::Int(24_000),
                required: false,
                help: "审计每批字节预算",
            },
            ParamSpec {
                key: "report",
                kind: ParamKind::Bool,
                default: ParamDefault::Bool(false),
                required: false,
                help: "输出覆盖报告",
            },
            ParamSpec {
                key: "classify",
                kind: ParamKind::Bool,
                default: ParamDefault::Bool(false),
                required: false,
                help: "页面分类",
            },
        ],
    },
    JobSpec {
        name: "maintain-wikitext",
        label: "wikitext 模板参数批量编辑 → 维基",
        wiki_access: WikiAccess::Write,
        resources: &["wiki", "out"],
        params: &[
            ParamSpec {
                key: "pages",
                kind: ParamKind::StrList,
                default: ParamDefault::None,
                required: true,
                help: "页面标题列表",
            },
            ParamSpec {
                key: "template",
                kind: ParamKind::Str,
                default: ParamDefault::None,
                required: true,
                help: "目标模板名（归一化匹配）",
            },
            ParamSpec {
                key: "set",
                kind: ParamKind::StrList,
                default: ParamDefault::None,
                required: false,
                help: "key=value 形式的设置项",
            },
            ParamSpec {
                key: "remove",
                kind: ParamKind::StrList,
                default: ParamDefault::None,
                required: false,
                help: "要删除的参数名",
            },
            ParamSpec {
                key: "output",
                kind: ParamKind::OptPath,
                default: ParamDefault::None,
                required: false,
                help: "报告与新文本输出目录",
            },
        ],
    },
    JobSpec {
        name: "create-redirect",
        label: "创建页面重定向",
        wiki_access: WikiAccess::Write,
        resources: &["wiki"],
        params: &[
            ParamSpec {
                key: "from",
                kind: ParamKind::Str,
                default: ParamDefault::None,
                required: true,
                help: "源页面标题（如 File:Wendy potion duration.png）",
            },
            ParamSpec {
                key: "to",
                kind: ParamKind::Str,
                default: ParamDefault::None,
                required: true,
                help: "目标页面标题（如 File:Wendy potion 3.png）",
            },
            ParamSpec {
                key: "summary",
                kind: ParamKind::OptStr,
                default: ParamDefault::None,
                required: false,
                help: "编辑摘要（可选）",
            },
        ],
    },
];

impl JobSpec {
    /// 按 canonical 名称查表。
    pub fn by_name(name: &str) -> Option<&'static JobSpec> {
        JOB_SPECS.iter().find(|s| s.name == name)
    }

    /// 是否可能写维基。
    pub fn touches_wiki(&self) -> bool {
        self.wiki_access == WikiAccess::Write
    }

    /// 按 serde 字段名查参数声明。
    pub fn param(&self, key: &str) -> Option<&'static ParamSpec> {
        self.params.iter().find(|p| p.key == key)
    }
}

#[cfg(test)]
mod tests {
    use super::super::JobKind;
    use super::*;

    /// 契约：spec 表与 JobKind 变体一一对应（数量、名称、serde tag）。
    #[test]
    fn contract_spec_table_covers_every_variant() {
        assert_eq!(JOB_SPECS.len(), 33);
        let mut names: Vec<&str> = JOB_SPECS.iter().map(|s| s.name).collect();
        names.sort_unstable();
        let count = names.len();
        names.dedup();
        assert_eq!(names.len(), count, "JOB_SPECS 存在重复名称");

        for kind in JobKind::all_variants() {
            let spec = kind.spec();
            let tag = serde_json::to_value(&kind).unwrap()["kind"]
                .as_str()
                .unwrap()
                .to_string();
            assert_eq!(
                tag,
                spec.name.replace('-', "_"),
                "spec `{}` 与 serde tag `{tag}` 不一致",
                spec.name
            );
        }
    }

    /// 契约：写维基的 job 必须声明 "wiki" 资源；只读/本地 job 不得声明。
    #[test]
    fn contract_wiki_resource_matches_access() {
        for spec in JOB_SPECS {
            assert_eq!(
                spec.resources.contains(&"wiki"),
                spec.touches_wiki(),
                "spec `{}` 的 wiki 资源与访问级别不一致",
                spec.name
            );
        }
    }

    /// 契约：必填参数必须有 required 标记（供 CLI/Web 校验）。
    #[test]
    fn contract_params_have_kinds() {
        for spec in JOB_SPECS {
            for p in spec.params {
                if p.required {
                    assert!(
                        matches!(
                            p.kind,
                            ParamKind::Str | ParamKind::Path | ParamKind::StrList
                        ),
                        "spec `{}` 参数 `{}` required 但类型不支持",
                        spec.name,
                        p.key
                    );
                }
                if let ParamDefault::Int(_) = p.default {
                    assert!(
                        matches!(p.kind, ParamKind::Usize),
                        "spec `{}` 参数 `{}` 默认值为整数但类型不是 Usize",
                        spec.name,
                        p.key
                    );
                }
            }
        }
    }
}
