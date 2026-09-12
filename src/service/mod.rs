pub mod dataset;
pub mod job_spec;
pub mod prefab_overrides;
pub mod skilltree_wiki;
pub mod snapshot_diff;
pub mod strings_wiki;
pub mod template_check;
pub mod upload_icons;
pub mod upload_image;
pub mod wiki_write;
pub mod wikitext_edit;

pub use job_spec::{JobSpec, WikiAccess, JOB_SPECS};

use crate::error::{Error, Result};
use crate::mapping::{compare_and_report, WikiDataConverter, WikiMapper};
use crate::models::{derive_station_aliases, PoEntry, StationAliasInputs, StationAliasReport};
use crate::parser::{
    extract_field_assignment_range, parse_crafting_filter_lists, parse_prototyper_trees,
    parse_tech_constants, RecipeParser,
};
use crate::platform::progress::{Reporter, WriteMode};
use crate::wiki::WikiClient;
use crate::{DstContext, TechReport};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use tracing::Instrument;

/// Everything the WebUI (or CLI) needs to describe one unit of work.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum JobKind {
    ParsePo {
        input: String,
        #[serde(default)]
        output: Option<String>,
        #[serde(default)]
        category: Option<String>,
    },
    MapNames {
        input: String,
        #[serde(default)]
        output: Option<String>,
        #[serde(default)]
        compare: Option<String>,
        #[serde(default)]
        merge: bool,
        #[serde(default)]
        version: Option<String>,
    },
    MapRecipes {
        input: String,
        #[serde(default)]
        output: Option<String>,
        #[serde(default)]
        compare: Option<String>,
        #[serde(default)]
        merge: bool,
        #[serde(default)]
        po_file: Option<String>,
        #[serde(default)]
        version: Option<String>,
    },
    MaintainItemTable {
        #[serde(default)]
        output: Option<String>,
        /// Optional scripts snapshot directory name.
        #[serde(default)]
        snapshot: Option<String>,
    },
    MaintainDstRecipes {
        #[serde(default)]
        output: Option<String>,
        #[serde(default)]
        snapshot: Option<String>,
    },
    MaintainCopyClip {
        #[serde(default)]
        r#type: Option<String>,
        #[serde(default)]
        output: Option<String>,
        #[serde(default)]
        snapshot: Option<String>,
    },
    /// 只读检查 `模板:Tech/dst` 与 `模板:制作栏图标` 对游戏数据的覆盖率；
    /// 输出控制台报告、`--report-json` 与可粘贴片段（`output`）。不写维基。
    MaintainTemplateCheck {
        #[serde(default)]
        output: Option<String>,
        #[serde(default)]
        snapshot: Option<String>,
        #[serde(default)]
        skip_icon_status: bool,
    },
    /// `maintain-strings`：解析游戏 `strings.pot`/`chinese_s.po`，变换为
    /// `模块:<V> Strings <LANG> <NN>` 桶页与 `Data:<V>_Strings_Index.json`，
    /// 并与现网逐桶对比；只写有变化的桶页，最后写索引（DryRun 只产本地报告）。
    MaintainStrings {
        /// 版本前缀（页面名与索引名），如 `DST`。
        #[serde(default = "default_strings_version")]
        version: String,
        #[serde(default)]
        snapshot: Option<String>,
        #[serde(default)]
        output: Option<String>,
        /// 跳过现网对比（无维基流量）。
        #[serde(default)]
        offline: bool,
        /// 无现有索引时的等分桶数。
        #[serde(default = "default_strings_buckets")]
        bucket_count: usize,
        /// 忽略现有索引边界，强制等量重切。
        #[serde(default)]
        rebalance: bool,
        /// Canary：最多写入 N 个桶页且不更新索引；0 = 不限制。
        #[serde(default)]
        limit: usize,
    },
    /// 把游戏 skilltree_<char>.lua 提取为 模块:Skilltree/<Char> 子页面的
    /// defs JSON（保留页内 metainfo 与 icon_url；`output` 同时写出
    /// Skilltree.js 渲染器与图片清单）。`character` 为子串过滤。
    SkilltreeWiki {
        #[serde(default)]
        character: Option<String>,
        #[serde(default)]
        output: Option<String>,
        #[serde(default)]
        snapshot: Option<String>,
    },
    /// 把技能树数据导出为本地 JSON 文件；纯本地，不访问维基。
    SkilltreeExport {
        #[serde(default)]
        character: Option<String>,
        #[serde(default)]
        output: Option<String>,
        #[serde(default)]
        snapshot: Option<String>,
    },
    /// 上传本地图片到维基（按素材目录自动补分类；同名重传可忽略警告）。
    UploadImage {
        path: String,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        description: Option<String>,
        #[serde(default)]
        comment: Option<String>,
        #[serde(default)]
        ignore_warnings: bool,
    },
    PrefabOverrides {
        input: String,
        #[serde(default)]
        output: Option<String>,
    },
    /// 扫描目录下全部 Lua 文件解析预制体重定向并合并（纯本地）。
    PrefabOverridesDir {
        /// Lua 目录；缺省 `DST__ROOT/data/databundles/scripts/prefabs`。
        #[serde(default)]
        input: Option<String>,
        #[serde(default)]
        output: Option<String>,
    },
    /// 只读审计 模块:ItemTable/PrefabOverrides：线上条目 × 本地推导做
    /// key/target 存在性验证，产出可推导/需人工补丁/可疑三张清单。
    PrefabOverridesAudit {
        /// 游戏脚本根目录；缺省 `DST__ROOT/data/databundles/scripts`。
        #[serde(default)]
        scripts: Option<String>,
        /// 线上页面原文文件；缺省从维基拉取（只读）。
        #[serde(default)]
        wiki_file: Option<String>,
        #[serde(default)]
        output: Option<String>,
    },
    /// 维护 模块:ItemTable/PrefabOverrides（当前只读）：本地推导渲染后与
    /// 线上 merge-preserving diff，产出更新/新增清单与本地页面文本，不写维基。
    MaintainPrefabOverrides {
        /// 游戏脚本根目录；缺省 `DST__ROOT/data/databundles/scripts`。
        #[serde(default)]
        scripts: Option<String>,
        /// 线上页面原文文件；缺省从维基拉取（只读）。
        #[serde(default)]
        wiki_file: Option<String>,
        /// 产物目录：`PrefabOverrides.lua` + `diff.json`。
        #[serde(default)]
        output: Option<String>,
    },
    /// Sync the game `scripts` tree after an update: archive the old tree as
    /// a snapshot, extract the new `scripts.zip`, record the version.
    /// Local-only; no wiki traffic.
    ScriptsSync {
        /// Sync even when the recorded version matches.
        #[serde(default)]
        force: bool,
        /// Report the planned actions without touching any file.
        #[serde(default)]
        dry_run: bool,
        /// Version state file (defaults to ./dst_version.txt).
        #[serde(default)]
        state_path: Option<String>,
    },
    /// 处理游戏图片资源（scripts-sync 的图片下半段）：两源盘点 → 解压
    /// images.zip → 内置 KTEX 解码（ktex-rs）→ 按 xml 切割，最终产物入
    /// 差异历史。结束后刷新图标元数据（名称表 + 维基上传状态查询）。
    ImagesSync {
        /// 忽略增量与幂等检查，全量重跑。
        #[serde(default)]
        force: bool,
        /// 只盘点并报告计划，不写任何文件。
        #[serde(default)]
        dry_run: bool,
        /// 跳过维基上传状态查询（默认查询；凭据缺失时自动跳过）。
        #[serde(default)]
        skip_wiki_status: bool,
    },
    /// 上传图标到维基：标题取映射表（按本地文件名）或 `STRINGS.NAMES`
    /// 英文名 + `.png`（描述按来源：物品栏 `[[分类:物品栏图标]]` / 制作栏
    /// `[[分类:制作栏图标]]`）；批量默认仅上传维基缺失的（需先运行
    /// images-sync 生成 `history/icon_meta.json`）；`file` + `title` 为
    /// 手动指定文件名上传。
    UploadIcons {
        /// 只上传首次加入该 build 的图标。
        #[serde(default)]
        build: Option<String>,
        /// 只上传指定来源（`inventory` / `crafting`）；缺省全部来源。
        #[serde(default)]
        source: Option<String>,
        /// 只上传单个图标文件名（如 `axe.png`，优先于 `build`）。
        #[serde(default)]
        file: Option<String>,
        /// 手动指定 wiki 文件名（需配合 `file`；写入映射表并上传）。
        #[serde(default)]
        title: Option<String>,
        /// 批量时仅上传维基上尚不存在的图标（缺省 true，与 CLI/Web 默认一致）。
        #[serde(default = "default_only_missing")]
        only_missing: bool,
        /// 同名重传发送 `ignorewarnings=1`。
        #[serde(default)]
        ignore_warnings: bool,
        /// 上传注释。
        #[serde(default)]
        comment: Option<String>,
    },
    /// 动画历史同步：更新后扫描 data/anim，归档 zip/dyn 到 ANIM__OUT_DIR。
    AnimSync {
        #[serde(default)]
        force: bool,
        #[serde(default)]
        dry_run: bool,
        #[serde(default)]
        label: Option<String>,
        #[serde(default)]
        out: Option<String>,
    },
    /// 对比两个动画目录的 zip/dyn 结构化 diff。
    AnimDiff {
        old: String,
        new: String,
        #[serde(default)]
        zip: Option<String>,
    },
    /// 构建 prefab 与动画文件的关联索引（纯本地，不做皮肤关联）。
    AnimIndex {
        scripts: String,
        #[serde(default)]
        anim: Option<String>,
        #[serde(default)]
        out: Option<String>,
    },
    /// 解析 skinprefabs.lua 生成 base_prefab → skins 皮肤索引（纯本地）。
    SkinIndex {
        /// 游戏脚本根目录；缺省取 DST__ROOT/data/databundles/scripts。
        #[serde(default)]
        scripts: Option<String>,
        #[serde(default)]
        anim: Option<String>,
        #[serde(default)]
        out: Option<String>,
    },
    /// Harvest the wiki main namespace into the local corpus tree
    /// (docs/WIKI_CORPUS_PLAN.md). Read-only against the wiki; `full`
    /// ignores `touched`-based incremental skipping.
    /// Snapshot diff + impact report (M1): read-only pipeline.
    UpdateScan {
        old: String,
        new: String,
        #[serde(default)]
        out: Option<String>,
        /// 语料 host 根目录（wikis/<host>/）：提供时附加 Layer B 定级摘要
        #[serde(default)]
        corpus: Option<String>,
        /// 输出 fn 标注骨架到该目录（prefabs 前 50 文件 + hound.lua）
        #[serde(default)]
        annotate: Option<String>,
    },
    /// Build the code association atlas (index + tuning) from a scripts root.
    UpdateIndex {
        root: String,
        #[serde(default)]
        out: Option<String>,
    },
    CorpusFetch {
        #[serde(default)]
        full: bool,
        #[serde(default)]
        dir: Option<String>,
        /// recentchanges 增量通道(§12);缺省仍为枚举对账
        #[serde(default)]
        rc: bool,
    },
    /// Rebuild derived corpus indexes from the local corpus tree
    /// (docs/CORPUS_CODE_ATLAS_CONTRACT.md). Local-only, no wiki traffic.
    CorpusIndex {
        #[serde(default)]
        dir: Option<String>,
        /// Optional code-side index.json for the join calibration report.
        #[serde(default)]
        join: Option<String>,
    },
    /// Page→Symbol 标注 CLI：生成证据包/Prompt，可选生成跨页一致性报告。
    SymbolAnnotate {
        root: String,
        corpus: String,
        #[serde(default = "default_symbol_limit")]
        limit: usize,
        #[serde(default)]
        out: Option<String>,
        #[serde(default)]
        verdicts: Option<String>,
        #[serde(default)]
        llm: bool,
        /// 每批最多交给 LLM 的页面数（0 = 不按页数设限）
        #[serde(default = "default_symbol_batch_pages")]
        batch_pages: usize,
        /// 每批渲染输入的字节预算（0 = 不按字节设限）
        #[serde(default = "default_symbol_batch_max_chars")]
        batch_max_chars: usize,
        /// 零候选证据的页面不送 LLM，本地合成 low-confidence missing 判定
        #[serde(default)]
        skip_no_fact_pages: bool,
    },
    /// M1:LLM 阅读符号源码生成 SymbolDoc 知识文档(本地,不写 wiki)
    KnowledgeScanSymbols {
        root: String,
        #[serde(default = "default_symbol_category")]
        category: String,
        #[serde(default = "default_knowledge_dir")]
        knowledge_dir: String,
        /// 提供则启用 pass2 语料归因
        #[serde(default)]
        corpus: Option<String>,
        #[serde(default = "default_sample_pages")]
        sample_pages: usize,
        #[serde(default = "default_symbol_limit")]
        limit: usize,
        #[serde(default)]
        force: bool,
        /// 并行处理的组件数;1 = 串行
        #[serde(default = "default_concurrency")]
        concurrency: usize,
        /// 仅对指定文件名词干跑 pass2;None = 全部
        #[serde(default)]
        pass2_names: Option<Vec<String>>,
        /// 仅选取指定文件名词干的符号;None = 按排序取 limit
        #[serde(default)]
        pick_names: Option<Vec<String>>,
        /// 不调 LLM:仅用 AutoInfobox 冷数据刷新现有文档的 auto_maintained
        #[serde(default)]
        refresh_auto: bool,
        /// pass2 二次确认(采样 ≥3 页但判空时复查)
        #[serde(default)]
        confirm_empty: bool,
    },
    /// page-assist:给定页面输出覆盖缺口建议清单(本地,不调 LLM)
    PageAssist {
        #[serde(default = "default_knowledge_dir")]
        knowledge_dir: String,
        page: Option<String>,
        #[serde(default)]
        all: bool,
        #[serde(default)]
        json: bool,
        /// 目标 3:编辑归因模式(需 corpus)
        #[serde(default)]
        attribute: bool,
        #[serde(default)]
        corpus: Option<String>,
    },
    /// M3:代码变更 → 脏文档 → 页面锚点交叉(确定性;--rescan 级联 LLM 重扫)
    KnowledgeSync {
        /// 旧快照(时间戳或目录名)
        old: String,
        /// 新快照(时间戳、目录名或 "current",默认 current)
        #[serde(default = "default_sync_new")]
        new: String,
        #[serde(default = "default_knowledge_dir")]
        knowledge_dir: String,
        #[serde(default)]
        rescan: bool,
        #[serde(default = "default_sync_limit")]
        limit: usize,
        /// 语料根目录;提供则启用 prefab→页面交叉
        #[serde(default)]
        corpus: Option<String>,
        /// Tier2:起草页面修订建议(需 corpus + LLM)
        #[serde(default)]
        draft: bool,
        /// Tier2 复核:裁决文件路径(对既有报告的建议逐条 approve/reject)
        #[serde(default)]
        review: Option<String>,
    },
    /// M2a:PageSymbolMap 确定性骨架(本地,不写 wiki、不调 LLM)
    KnowledgeScanWiki {
        root: String,
        #[serde(default = "default_knowledge_dir")]
        knowledge_dir: String,
        corpus: String,
        #[serde(default)]
        audit: bool,
        #[serde(default)]
        audit_symbols: Option<Vec<String>>,
        #[serde(default = "default_audit_max_pages")]
        audit_max_pages: usize,
        #[serde(default = "default_audit_batch_pages")]
        audit_batch_pages: usize,
        #[serde(default = "default_audit_batch_max_chars")]
        audit_batch_max_chars: usize,
        #[serde(default)]
        report: bool,
        #[serde(default)]
        classify: bool,
    },
    /// `maintain-wikitext`：wikitext 解析器驱动的模板参数外科手术式批量
    /// 编辑（`set`/`remove`），逐页 diff；DryRun 只产报告不写维基。
    MaintainWikitext {
        /// 页面标题列表。
        #[serde(default)]
        pages: Vec<String>,
        /// 目标模板名（归一化匹配）。
        template: String,
        /// `key=value` 形式的设置项。
        #[serde(default)]
        set: Vec<String>,
        /// 要删除的参数名。
        #[serde(default)]
        remove: Vec<String>,
        /// 报告与新文本的输出目录。
        #[serde(default)]
        output: Option<String>,
    },
}

fn default_only_missing() -> bool {
    true
}

fn default_sync_new() -> String {
    "current".to_string()
}

fn default_sync_limit() -> usize {
    20
}

fn default_audit_max_pages() -> usize {
    60
}

fn default_audit_batch_pages() -> usize {
    20
}

fn default_audit_batch_max_chars() -> usize {
    24_000
}

fn default_symbol_category() -> String {
    "component".to_string()
}

fn default_concurrency() -> usize {
    1
}

fn default_knowledge_dir() -> String {
    "knowledge".to_string()
}

fn default_sample_pages() -> usize {
    8
}

fn default_symbol_limit() -> usize {
    20
}

fn default_symbol_batch_pages() -> usize {
    40
}

fn default_symbol_batch_max_chars() -> usize {
    crate::update::DEFAULT_BATCH_MAX_CHARS
}

fn default_strings_version() -> String {
    "DST".to_string()
}

fn default_strings_buckets() -> usize {
    strings_wiki::DEFAULT_BUCKETS
}

impl JobKind {
    /// 对应的 [`JobSpec`] 声明（canonical name/wiki_access/resources/params
    /// 的唯一来源）。
    pub fn spec(&self) -> &'static job_spec::JobSpec {
        match self {
            JobKind::ParsePo { .. } => &job_spec::JOB_SPECS[0],
            JobKind::MapNames { .. } => &job_spec::JOB_SPECS[1],
            JobKind::MapRecipes { .. } => &job_spec::JOB_SPECS[2],
            JobKind::MaintainItemTable { .. } => &job_spec::JOB_SPECS[3],
            JobKind::MaintainDstRecipes { .. } => &job_spec::JOB_SPECS[4],
            JobKind::MaintainCopyClip { .. } => &job_spec::JOB_SPECS[5],
            JobKind::MaintainTemplateCheck { .. } => &job_spec::JOB_SPECS[6],
            JobKind::MaintainStrings { .. } => &job_spec::JOB_SPECS[7],
            JobKind::SkilltreeWiki { .. } => &job_spec::JOB_SPECS[8],
            JobKind::SkilltreeExport { .. } => &job_spec::JOB_SPECS[9],
            JobKind::UploadImage { .. } => &job_spec::JOB_SPECS[10],
            JobKind::PrefabOverrides { .. } => &job_spec::JOB_SPECS[11],
            JobKind::PrefabOverridesDir { .. } => &job_spec::JOB_SPECS[12],
            JobKind::PrefabOverridesAudit { .. } => &job_spec::JOB_SPECS[13],
            JobKind::MaintainPrefabOverrides { .. } => &job_spec::JOB_SPECS[14],
            JobKind::ScriptsSync { .. } => &job_spec::JOB_SPECS[15],
            JobKind::ImagesSync { .. } => &job_spec::JOB_SPECS[16],
            JobKind::UploadIcons { .. } => &job_spec::JOB_SPECS[17],
            JobKind::AnimSync { .. } => &job_spec::JOB_SPECS[18],
            JobKind::AnimDiff { .. } => &job_spec::JOB_SPECS[19],
            JobKind::AnimIndex { .. } => &job_spec::JOB_SPECS[20],
            JobKind::SkinIndex { .. } => &job_spec::JOB_SPECS[21],
            JobKind::UpdateScan { .. } => &job_spec::JOB_SPECS[22],
            JobKind::UpdateIndex { .. } => &job_spec::JOB_SPECS[23],
            JobKind::CorpusFetch { .. } => &job_spec::JOB_SPECS[24],
            JobKind::CorpusIndex { .. } => &job_spec::JOB_SPECS[25],
            JobKind::SymbolAnnotate { .. } => &job_spec::JOB_SPECS[26],
            JobKind::KnowledgeScanSymbols { .. } => &job_spec::JOB_SPECS[27],
            JobKind::PageAssist { .. } => &job_spec::JOB_SPECS[28],
            JobKind::KnowledgeSync { .. } => &job_spec::JOB_SPECS[29],
            JobKind::KnowledgeScanWiki { .. } => &job_spec::JOB_SPECS[30],
            JobKind::MaintainWikitext { .. } => &job_spec::JOB_SPECS[31],
        }
    }

    /// Human readable name used in job listings (canonical, == CLI name).
    pub fn name(&self) -> &'static str {
        self.spec().name
    }

    /// 中文显示名（Web 前端 / 日志）。
    pub fn label(&self) -> &'static str {
        self.spec().label
    }

    /// 资源竞争域（WebUI JobManager 跨任务互斥键）。
    pub fn resources(&self) -> &'static [&'static str] {
        self.spec().resources
    }

    /// Whether this job may edit pages on the wiki (needs explicit confirm).
    pub fn touches_wiki(&self) -> bool {
        self.spec().touches_wiki()
    }

    /// 每个变体一个最小实例（字段取占位值），供契约测试与文档枚举。
    ///
    /// 契约（`service::tests` 与 `commands::tests` 各自强制）：
    /// - `name()` 在全集中唯一，且 == serde tag 的 kebab 形式；
    /// - CLI 子命令名集合（除 `serve`）== `name()` 集合；
    /// - 前端 `JOB_DEFS` 的键 ⊆ serde tag 集合。
    pub fn all_variants() -> Vec<JobKind> {
        vec![
            JobKind::ParsePo {
                input: String::new(),
                output: None,
                category: None,
            },
            JobKind::MapNames {
                input: String::new(),
                output: None,
                compare: None,
                merge: false,
                version: None,
            },
            JobKind::MapRecipes {
                input: String::new(),
                output: None,
                compare: None,
                merge: false,
                po_file: None,
                version: None,
            },
            JobKind::MaintainItemTable {
                output: None,
                snapshot: None,
            },
            JobKind::MaintainDstRecipes {
                output: None,
                snapshot: None,
            },
            JobKind::MaintainCopyClip {
                r#type: None,
                output: None,
                snapshot: None,
            },
            JobKind::MaintainTemplateCheck {
                output: None,
                snapshot: None,
                skip_icon_status: false,
            },
            JobKind::MaintainStrings {
                version: default_strings_version(),
                snapshot: None,
                output: None,
                offline: false,
                bucket_count: default_strings_buckets(),
                rebalance: false,
                limit: 0,
            },
            JobKind::SkilltreeWiki {
                character: None,
                output: None,
                snapshot: None,
            },
            JobKind::SkilltreeExport {
                character: None,
                output: None,
                snapshot: None,
            },
            JobKind::UploadImage {
                path: String::new(),
                name: None,
                description: None,
                comment: None,
                ignore_warnings: false,
            },
            JobKind::PrefabOverrides {
                input: String::new(),
                output: None,
            },
            JobKind::PrefabOverridesDir {
                input: None,
                output: None,
            },
            JobKind::PrefabOverridesAudit {
                scripts: None,
                wiki_file: None,
                output: None,
            },
            JobKind::MaintainPrefabOverrides {
                scripts: None,
                wiki_file: None,
                output: None,
            },
            JobKind::ScriptsSync {
                force: false,
                dry_run: true,
                state_path: None,
            },
            JobKind::ImagesSync {
                force: false,
                dry_run: true,
                skip_wiki_status: true,
            },
            JobKind::UploadIcons {
                build: None,
                source: None,
                file: None,
                title: None,
                only_missing: false,
                ignore_warnings: false,
                comment: None,
            },
            JobKind::AnimSync {
                force: false,
                dry_run: true,
                label: None,
                out: None,
            },
            JobKind::AnimDiff {
                old: String::new(),
                new: String::new(),
                zip: None,
            },
            JobKind::AnimIndex {
                scripts: String::new(),
                anim: None,
                out: None,
            },
            JobKind::SkinIndex {
                scripts: None,
                anim: None,
                out: None,
            },
            JobKind::UpdateScan {
                old: String::new(),
                new: String::new(),
                out: None,
                corpus: None,
                annotate: None,
            },
            JobKind::UpdateIndex {
                root: String::new(),
                out: None,
            },
            JobKind::CorpusFetch {
                full: false,
                dir: None,
                rc: false,
            },
            JobKind::CorpusIndex {
                dir: None,
                join: None,
            },
            JobKind::SymbolAnnotate {
                root: String::new(),
                corpus: String::new(),
                limit: default_symbol_limit(),
                out: None,
                verdicts: None,
                llm: false,
                batch_pages: default_symbol_batch_pages(),
                batch_max_chars: default_symbol_batch_max_chars(),
                skip_no_fact_pages: false,
            },
            JobKind::KnowledgeScanSymbols {
                root: String::new(),
                category: default_symbol_category(),
                knowledge_dir: default_knowledge_dir(),
                corpus: None,
                sample_pages: default_sample_pages(),
                limit: default_symbol_limit(),
                force: false,
                concurrency: default_concurrency(),
                pass2_names: None,
                pick_names: None,
                refresh_auto: false,
                confirm_empty: false,
            },
            JobKind::PageAssist {
                knowledge_dir: default_knowledge_dir(),
                page: None,
                all: false,
                json: false,
                attribute: false,
                corpus: None,
            },
            JobKind::KnowledgeSync {
                old: String::new(),
                new: default_sync_new(),
                knowledge_dir: default_knowledge_dir(),
                rescan: false,
                limit: default_sync_limit(),
                corpus: None,
                draft: false,
                review: None,
            },
            JobKind::KnowledgeScanWiki {
                root: String::new(),
                knowledge_dir: default_knowledge_dir(),
                corpus: String::new(),
                audit: false,
                audit_symbols: None,
                audit_max_pages: default_audit_max_pages(),
                audit_batch_pages: default_audit_batch_pages(),
                audit_batch_max_chars: default_audit_batch_max_chars(),
                report: false,
                classify: false,
            },
            JobKind::MaintainWikitext {
                pages: Vec::new(),
                template: String::new(),
                set: Vec::new(),
                remove: Vec::new(),
                output: None,
            },
        ]
    }
}

pub(crate) fn make_ctx(snapshot: &Option<String>) -> Result<DstContext> {
    let dst_root = crate::platform::config::dst_root_str()?;
    DstContext::new(dst_root, snapshot.clone())
}

fn opt_path(p: &Option<String>) -> Option<PathBuf> {
    p.as_ref().map(PathBuf::from)
}

fn path_str(p: &str) -> Result<&str> {
    Ok(p)
}

/// Executes a job to completion. Long running jobs should be spawned on a
/// dedicated tokio task by the caller (the WebUI JobManager does this).
pub async fn execute_job(kind: &JobKind, reporter: &dyn Reporter) -> Result<serde_json::Value> {
    execute_job_with_mode(kind, reporter, WriteMode::Interactive).await
}

/// Like [`execute_job`], but with an explicit [`WriteMode`] controlling how
/// (and whether) wiki writes happen.
pub async fn execute_job_with_mode(
    kind: &JobKind,
    reporter: &dyn Reporter,
    mode: WriteMode,
) -> Result<serde_json::Value> {
    reporter.stage(&format!("{}（写入模式：{}）", kind.name(), mode.name()));
    // `Instrument` instead of `enter()`: an EnteredSpan is !Send and would
    // make this future unusable with tokio::spawn (WebUI job manager).
    execute_job_inner(kind, reporter, mode)
        .instrument(tracing::info_span!(
            "job",
            job = kind.name(),
            write_mode = mode.name()
        ))
        .await
}

async fn execute_job_inner(
    kind: &JobKind,
    reporter: &dyn Reporter,
    mode: WriteMode,
) -> Result<serde_json::Value> {
    match kind {
        JobKind::ParsePo {
            input,
            output,
            category,
        } => run_parse_po(input, opt_path(output), category.clone(), reporter).await,
        JobKind::MapNames {
            input,
            output,
            compare,
            merge,
            version,
        } => {
            run_map_names(
                input,
                opt_path(output),
                compare.clone(),
                *merge,
                version.clone(),
                reporter,
            )
            .await
        }
        JobKind::MapRecipes {
            input,
            output,
            compare,
            merge,
            po_file,
            version,
        } => {
            run_map_recipes(
                input,
                opt_path(output),
                compare.clone(),
                *merge,
                po_file.clone(),
                version.clone(),
                reporter,
            )
            .await
        }
        JobKind::MaintainItemTable { output, snapshot } => {
            run_maintain_item_table(opt_path(output), snapshot.clone(), reporter, mode).await
        }
        JobKind::MaintainDstRecipes { output, snapshot } => {
            run_maintain_dst_recipes(opt_path(output), snapshot.clone(), reporter, mode).await
        }
        JobKind::MaintainCopyClip {
            r#type,
            output,
            snapshot,
        } => {
            run_maintain_copyclip(
                r#type.as_deref(),
                opt_path(output),
                snapshot.clone(),
                reporter,
                mode,
            )
            .await
        }
        JobKind::MaintainTemplateCheck {
            output,
            snapshot,
            skip_icon_status,
        } => {
            template_check::run(
                &template_check::TemplateCheckParams {
                    output: opt_path(output),
                    snapshot: snapshot.clone(),
                    skip_icon_status: *skip_icon_status,
                },
                reporter,
            )
            .await
        }
        JobKind::MaintainStrings {
            version,
            snapshot,
            output,
            offline,
            bucket_count,
            rebalance,
            limit,
        } => {
            strings_wiki::run(
                &strings_wiki::StringsWikiParams {
                    version: version.clone(),
                    snapshot: snapshot.clone(),
                    output: opt_path(output),
                    offline: *offline,
                    bucket_count: *bucket_count,
                    rebalance: *rebalance,
                    limit: *limit,
                },
                reporter,
                mode,
            )
            .await
        }
        JobKind::MaintainWikitext {
            pages,
            template,
            set,
            remove,
            output,
        } => {
            wikitext_edit::run(
                &wikitext_edit::WikitextEditParams {
                    pages: pages.clone(),
                    template: template.clone(),
                    set: set.clone(),
                    remove: remove.clone(),
                    output: opt_path(output),
                },
                reporter,
                mode,
            )
            .await
        }
        JobKind::PrefabOverrides { input, output } => {
            prefab_overrides::run_prefab_overrides(input, opt_path(output), reporter).await
        }
        JobKind::PrefabOverridesDir { input, output } => {
            prefab_overrides::run_prefab_overrides_dir(input.as_deref(), opt_path(output), reporter)
                .await
        }
        JobKind::PrefabOverridesAudit {
            scripts,
            wiki_file,
            output,
        } => {
            prefab_overrides::audit::run_prefab_overrides_audit(
                scripts.as_deref(),
                wiki_file.as_deref(),
                opt_path(output),
                reporter,
            )
            .await
        }
        JobKind::MaintainPrefabOverrides {
            scripts,
            wiki_file,
            output,
        } => {
            prefab_overrides::maintain::run_maintain_prefab_overrides(
                scripts.as_deref(),
                wiki_file.as_deref(),
                opt_path(output),
                reporter,
            )
            .await
        }
        JobKind::SkilltreeWiki {
            character,
            output,
            snapshot,
        } => {
            skilltree_wiki::run_skilltree_wiki(
                character.clone(),
                output.clone(),
                snapshot.clone(),
                reporter,
                mode,
            )
            .await
        }
        JobKind::SkilltreeExport {
            character,
            output,
            snapshot,
        } => {
            skilltree_wiki::run_skilltree_export(
                character.clone(),
                opt_path(output),
                snapshot.clone(),
                reporter,
            )
            .await
        }
        JobKind::UploadImage {
            path,
            name,
            description,
            comment,
            ignore_warnings,
        } => {
            upload_image::run_upload_image(
                std::path::Path::new(path),
                name.as_deref(),
                description.as_deref(),
                comment.as_deref(),
                *ignore_warnings,
                reporter,
                mode,
            )
            .await
        }
        JobKind::ScriptsSync {
            force,
            dry_run,
            state_path,
        } => run_scripts_sync(*force, *dry_run, state_path.as_deref(), reporter).await,
        JobKind::ImagesSync {
            force,
            dry_run,
            skip_wiki_status,
        } => run_images_sync(*force, *dry_run, *skip_wiki_status, reporter).await,
        JobKind::UploadIcons {
            build,
            source,
            file,
            title,
            only_missing,
            ignore_warnings,
            comment,
        } => {
            upload_icons::run_upload_icons(
                &upload_icons::UploadIconsParams {
                    build: build.clone(),
                    source: source.clone(),
                    file: file.clone(),
                    title: title.clone(),
                    only_missing: *only_missing,
                    ignore_warnings: *ignore_warnings,
                    comment: comment.clone(),
                },
                reporter,
                mode,
            )
            .await
        }
        JobKind::AnimSync {
            force,
            dry_run,
            label,
            out,
        } => run_anim_sync(*force, *dry_run, label.clone(), opt_path(out), reporter).await,
        JobKind::AnimDiff { old, new, zip } => {
            run_anim_diff(
                PathBuf::from(old),
                PathBuf::from(new),
                zip.clone(),
                reporter,
            )
            .await
        }
        JobKind::AnimIndex { scripts, anim, out } => {
            run_anim_index(scripts, opt_path(anim), opt_path(out), reporter).await
        }
        JobKind::SkinIndex { scripts, anim, out } => {
            run_skin_index(scripts.as_deref(), opt_path(anim), opt_path(out), reporter)
        }
        JobKind::CorpusFetch { full, dir, rc } => {
            run_corpus_fetch(*full, *rc, dir.as_deref(), reporter, mode).await
        }
        JobKind::CorpusIndex { dir, join } => {
            run_corpus_index(dir.as_deref(), join.as_deref(), reporter, mode).await
        }
        JobKind::UpdateIndex { root, out } => run_update_index(root, opt_path(out), reporter).await,
        JobKind::UpdateScan {
            old,
            new,
            out,
            corpus,
            annotate,
        } => {
            run_update_scan(
                old,
                new,
                opt_path(out),
                corpus.as_deref(),
                annotate.as_deref(),
                reporter,
            )
            .await
        }
        JobKind::KnowledgeScanSymbols {
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
            crate::knowledge::run_scan_symbols(
                &crate::knowledge::ScanSymbolsParams {
                    scripts_root: root.clone(),
                    category: category.clone(),
                    knowledge_root: knowledge_dir.clone(),
                    corpus: corpus.clone(),
                    sample_pages: *sample_pages,
                    limit: *limit,
                    force: *force,
                    concurrency: *concurrency,
                    pass2_names: pass2_names.clone(),
                    pick_names: pick_names.clone(),
                    refresh_auto: *refresh_auto,
                    confirm_empty: *confirm_empty,
                },
                reporter,
            )
            .await
        }
        JobKind::KnowledgeSync {
            old,
            new,
            knowledge_dir,
            rescan,
            limit,
            corpus,
            draft,
            review,
        } => {
            crate::knowledge::run_knowledge_sync(
                &crate::knowledge::SyncParams {
                    old: old.clone(),
                    new: new.clone(),
                    knowledge_dir: knowledge_dir.clone(),
                    rescan: *rescan,
                    limit: *limit,
                    corpus: corpus.clone(),
                    draft: *draft,
                    review: review.clone(),
                },
                reporter,
            )
            .await
        }
        JobKind::PageAssist {
            knowledge_dir,
            page,
            all,
            json,
            attribute,
            corpus,
        } => {
            crate::knowledge::run_page_assist(
                &crate::knowledge::PageAssistParams {
                    knowledge_dir: knowledge_dir.clone(),
                    page: page.clone(),
                    all: *all,
                    json: *json,
                    attribute: *attribute,
                    corpus: corpus.clone(),
                },
                reporter,
            )
            .await
        }
        JobKind::KnowledgeScanWiki {
            root,
            knowledge_dir,
            corpus,
            audit,
            audit_symbols,
            audit_max_pages,
            audit_batch_pages,
            audit_batch_max_chars,
            report,
            classify,
        } => {
            crate::knowledge::run_scan_wiki(
                &crate::knowledge::ScanWikiParams {
                    scripts_root: root.clone(),
                    knowledge_dir: knowledge_dir.clone(),
                    corpus: corpus.clone(),
                    audit: *audit,
                    audit_symbols: audit_symbols.clone(),
                    audit_max_pages: *audit_max_pages,
                    audit_batch_pages: *audit_batch_pages,
                    audit_batch_max_chars: *audit_batch_max_chars,
                    report: *report,
                    classify: *classify,
                },
                reporter,
            )
            .await
        }
        JobKind::SymbolAnnotate {
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
            run_symbol_annotate(
                &SymbolAnnotateParams {
                    root: root.clone(),
                    corpus: corpus.clone(),
                    limit: *limit,
                    out: out.clone(),
                    verdicts: verdicts.clone(),
                    llm: *llm,
                    batch_pages: *batch_pages,
                    batch_max_chars: *batch_max_chars,
                    skip_no_fact_pages: *skip_no_fact_pages,
                },
                reporter,
            )
            .await
        }
    }
}

// ---------------------------------------------------------------------------
// Local-file commands (no game dir required)
// ---------------------------------------------------------------------------

fn cached_scan_summary(out_dir: &std::path::Path) -> Result<serde_json::Value> {
    let raw = std::fs::read_to_string(out_dir.join("impact.json"))?;
    let v: serde_json::Value = serde_json::from_str(&raw)?;
    let report = v.get("report").cloned().unwrap_or_default();
    let tier0 = v.get("tier0").cloned().unwrap_or_default();
    Ok(serde_json::json!({
        "cached": true,
        "output_dir": out_dir.display().to_string(),
        "old": report.get("old_id").cloned().unwrap_or_default(),
        "new": report.get("new_id").cloned().unwrap_or_default(),
        "changed_files": report.get("files").and_then(|x| x.as_array()).map(|a| a.len()).unwrap_or(0),
        "affected_entities": report.get("affected_entities").and_then(|x| x.as_array()).map(|a| a.len()).unwrap_or(0),
        "tuning_added": report.get("tuning").and_then(|t| t.get("added")).and_then(|x| x.as_array()).map(|a| a.len()).unwrap_or(0),
        "tuning_removed": report.get("tuning").and_then(|t| t.get("removed")).and_then(|x| x.as_array()).map(|a| a.len()).unwrap_or(0),
        "tuning_changed": report.get("tuning").and_then(|t| t.get("changed")).and_then(|x| x.as_object()).map(|m| m.len()).unwrap_or(0),
        "layer_b": report.get("layer_b").cloned().unwrap_or(serde_json::Value::Null),
        "tier0_hits": tier0.as_array().map(|a| a.len()).unwrap_or(0),
    }))
}

/// `update-scan`: snapshot diff -> association attribution -> Tier0 rules.
///
/// Read-only: writes `impact.json` + `changes.patch` under the output dir.
async fn run_update_scan(
    old_id: &str,
    new_id: &str,
    out: Option<PathBuf>,
    corpus: Option<&str>,
    annotate: Option<&str>,
    reporter: &dyn Reporter,
) -> Result<serde_json::Value> {
    reporter.stage("快照差异与影响评估");
    let store = crate::update::SnapshotStore::from_env()?;
    let old_root = store.resolve(old_id);
    let new_root = if new_id == "current" {
        store.current_dir()
    } else {
        store.resolve(new_id)
    };
    if !old_root.is_dir() || !new_root.is_dir() {
        return Err(crate::error::Error::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!(
                "snapshot dirs missing: {} or {}",
                old_root.display(),
                new_root.display()
            ),
        )));
    }

    let out_dir = out.unwrap_or_else(|| {
        PathBuf::from("output")
            .join("scan")
            .join(format!("{old_id}_{new_id}"))
    });
    std::fs::create_dir_all(&out_dir)?;
    if let Some(state) = crate::update::state::ScanState::load(&out_dir) {
        if state.old_id == old_id && state.new_id == new_id && state.status == "completed" {
            reporter.log("检测到已完成 state.json，跳过重算".to_string());
            reporter.log(format!("输出目录 {}", out_dir.display()));
            return cached_scan_summary(&out_dir);
        }
    }
    let mut state = crate::update::state::ScanState::new(old_id, new_id);
    state.save(&out_dir)?;

    reporter.log(format!(
        "diff: {} <-> {}",
        old_root.display(),
        new_root.display()
    ));
    let diff = crate::update::TreeDiff::diff_trees(&old_root, &new_root)?;
    state.mark_stage("diff", true, &out_dir)?;
    reporter.log(format!("变更文件 {} 个", diff.files.len()));

    reporter.stage("构建关联索引（新树）");
    let atlas = crate::update::build_atlas_from_dir(&new_root)?;
    state.mark_stage("atlas", true, &out_dir)?;

    let old_tuning_src = std::fs::read_to_string(old_root.join("tuning.lua")).ok();
    let old_tuning = old_tuning_src
        .as_deref()
        .map(crate::update::index::tuning::build_tuning)
        .transpose()?;
    let mut report = crate::update::build_report(
        &diff,
        &atlas.index,
        old_tuning.as_ref(),
        Some(&atlas.tuning),
        old_id,
        new_id,
    );

    // Layer B (report-only): pair loot facts across snapshots and grade the
    // changes against the local corpus. Read-only; skipped when no corpus
    // dir was supplied.
    let mut graded_layer_b: Option<Vec<crate::update::GradedChange>> = None;
    if let Some(corpus_root) = corpus {
        reporter.stage("Layer B 定级（loot 配对，只读）");
        reporter.log("构建旧树索引（loot 配对需要）".to_string());
        let old_atlas = crate::update::build_atlas_from_dir(&old_root)?;
        let mut changes =
            crate::update::pair_loot_changes(&old_atlas.index.loot, &atlas.index.loot);
        let old_stats =
            crate::update::collect_stat_records(&old_root, &old_atlas.index, &old_atlas.tuning);
        let new_stats = crate::update::collect_stat_records(&new_root, &atlas.index, &atlas.tuning);
        let stat_changes = crate::update::pair_stat_changes(&old_stats, &new_stats);
        reporter.log(format!("配对出数值变更 {} 条", stat_changes.len()));
        let old_consts = crate::update::collect_brain_consts(&old_root)?;
        let new_consts = crate::update::collect_brain_consts(&new_root)?;
        let const_changes = crate::update::pair_const_changes(&old_consts, &new_consts);
        reporter.log(format!("配对出行为常量变更 {} 条", const_changes.len()));
        changes.extend(const_changes);
        changes.extend(stat_changes);
        reporter.log(format!("配对变更合计 {} 条", changes.len()));
        let view = crate::update::CorpusPageView::load(std::path::Path::new(corpus_root))?;
        let graded = crate::update::grade_changes(&changes, &view);
        report.layer_b = Some(crate::update::grade::LayerBSummary::from(graded.as_slice()));
        reporter.log(format!(
            "Layer B: {} 条定级 {:?}",
            graded.len(),
            crate::update::summarize_grades(&graded)
        ));
        graded_layer_b = Some(graded);
        state.mark_stage("layer_b", true, &out_dir)?;
    }

    let changed_paths: Vec<String> = report.files.iter().map(|f| f.path.clone()).collect();
    let tier0 = crate::update::evaluate_rules(&crate::update::default_rules(), &changed_paths);
    for hit in &tier0 {
        reporter.log(format!(
            "Tier0 [{}] {} 文件 → jobs: {}",
            hit.label,
            hit.files.len(),
            if hit.jobs.is_empty() {
                "(无自动作业)".to_string()
            } else {
                hit.jobs.join(", ")
            }
        ));
    }

    // Persist draft-eligible suggestions for human comparison (L1).
    if let Some(annotate_dir) = annotate {
        reporter.stage("fn 标注骨架生成");
        // hound.lua + 按字母序前 50 个 prefab 文件
        let mut files: Vec<&str> = atlas
            .index
            .fn_ranges
            .keys()
            .filter(|f| f.starts_with("prefabs/") && f.ends_with(".lua"))
            .map(|s| s.as_str())
            .collect();
        files.sort();
        let mut chosen: Vec<&str> = files.clone();
        if !chosen.contains(&"prefabs/hound.lua") {
            chosen.insert(0, "prefabs/hound.lua");
        }
        let written = crate::update::batch_annotate(
            &atlas.index,
            &std::path::PathBuf::from(annotate_dir),
            &chosen,
        )?;
        reporter.log(format!(
            "标注骨架 {} 个文件 → {}",
            written.len(),
            annotate_dir
        ));
    }

    if let Some(graded) = &graded_layer_b {
        let path = out_dir.join("layer_b_rows.json");
        crate::platform::fs::write_json_atomic(&path, &graded)?;
        let drafts = graded
            .iter()
            .filter(|g| g.tier == crate::update::GradeTier::SuggestDraft)
            .count();

        // Formal create-check list: entities the pipeline believes need a wiki
        // page but currently have no page mapping.
        let create_checks: Vec<&crate::update::GradedChange> = graded
            .iter()
            .filter(|g| g.tier == crate::update::GradeTier::CreateCheck)
            .collect();
        let cc_json = out_dir.join("create_check.json");
        crate::platform::fs::write_json_atomic(&cc_json, &create_checks)?;
        let mut cc_md = String::from("# Create-Check 清单\n\n");
        for g in &create_checks {
            cc_md.push_str(&format!(
                "- `{}` {}：{}\n",
                g.prefab, g.field, g.change_text
            ));
        }
        let cc_md_path = out_dir.join("create_check.md");
        std::fs::write(&cc_md_path, cc_md)?;
        reporter.log(format!(
            "Layer B 明细 {} 条（建议 {}，create_check {}）→ {}",
            graded.len(),
            drafts,
            create_checks.len(),
            path.display()
        ));
        reporter.log(format!(
            "Create-Check 清单 {} 条 → {} / {}",
            create_checks.len(),
            cc_json.display(),
            cc_md_path.display()
        ));
    }

    let summary = serde_json::json!({
        "old": report.old_id,
        "new": report.new_id,
        "changed_files": report.files.len(),
        "affected_entities": report.affected_entities.len(),
        "tuning_added": report.tuning.added.len(),
        "layer_b": report.layer_b,
        "tuning_removed": report.tuning.removed.len(),
        "tuning_changed": report.tuning.changed.len(),
        "tier0_hits": tier0.len(),
    });
    crate::platform::fs::write_json_atomic(
        &out_dir.join("impact.json"),
        &serde_json::json!({
            "report": report,
            "tier0": tier0,
        }),
    )?;
    let patch = diff.to_patch(&old_root, &new_root)?;
    std::fs::write(out_dir.join("changes.patch"), &patch)?;

    reporter.log(format!("受影响实体 {}", report.affected_entities.len()));
    reporter.log(format!("已写入 {}", out_dir.display()));
    state.complete(&out_dir)?;

    Ok(summary)
}

/// `update-index`: build the association atlas and cache it under
/// `output/atlas/<build>/` (or an explicit `--out` directory).
///
/// Purely local: reads game scripts, writes two JSON artifacts, no wiki I/O.
async fn run_update_index(
    root: &str,
    out: Option<PathBuf>,
    reporter: &dyn Reporter,
) -> Result<serde_json::Value> {
    reporter.stage("构建代码关联索引");
    let root_path = std::path::Path::new(root);
    let atlas = crate::update::build_atlas_from_dir(root_path)?;

    let build_dir = atlas
        .build_id
        .clone()
        .or_else(|| {
            // Snapshot directories follow databundles/scripts_<yyyymmddhhmm>;
            // fall back to their timestamp so caches land per-build.
            root_path
                .file_name()
                .and_then(std::ffi::OsStr::to_str)
                .filter(|n| n.starts_with("scripts_"))
                .map(|n| n.trim_start_matches("scripts_").to_string())
        })
        .unwrap_or_else(|| "unknown".to_string());
    let out_dir = out.unwrap_or_else(|| PathBuf::from("output").join("atlas").join(build_dir));
    std::fs::create_dir_all(&out_dir)?;

    let index_path = out_dir.join("index.json");
    crate::platform::fs::write_json_atomic(&index_path, &atlas.index)?;
    let tuning_path = out_dir.join("tuning.json");
    crate::platform::fs::write_json_atomic(&tuning_path, &atlas.tuning)?;

    reporter.log(format!(
        "{} 文件 / {} 边 / {} 行为实参 / TUNING 标量 {}",
        atlas.index.scanned_files,
        atlas.index.edges.len(),
        atlas.index.behaviour_calls.len(),
        atlas.tuning.values.len()
    ));
    reporter.log(format!("已写入 {}", out_dir.display()));

    Ok(serde_json::json!({
        "schema_version": atlas.schema_version,
        "build_id": atlas.build_id,
        "scanned_files": atlas.index.scanned_files,
        "edges": atlas.index.edges.len(),
        "behaviour_calls": atlas.index.behaviour_calls.len(),
        "unresolved": atlas.index.unresolved.len(),
        "tuning_values": atlas.tuning.values.len(),
        "output": out_dir,
    }))
}

/// `symbol-annotate`: build Page→Symbol evidence packs, render prompts, and
/// optionally consume LLM/human verdicts to produce coverage reports.
///
/// Local-only; never writes the wiki.
struct SymbolAnnotateParams {
    root: String,
    corpus: String,
    limit: usize,
    out: Option<String>,
    verdicts: Option<String>,
    llm: bool,
    batch_pages: usize,
    batch_max_chars: usize,
    skip_no_fact_pages: bool,
}

async fn run_symbol_annotate(
    params: &SymbolAnnotateParams,
    reporter: &dyn Reporter,
) -> Result<serde_json::Value> {
    let SymbolAnnotateParams {
        root,
        corpus,
        limit,
        out,
        verdicts,
        llm,
        batch_pages,
        batch_max_chars,
        skip_no_fact_pages,
    } = params;
    let out = opt_path(out);
    let verdicts = verdicts.as_deref();
    let (root, corpus) = (root.as_str(), corpus.as_str());
    reporter.stage("构建代码关联索引");
    let atlas = crate::update::build_atlas_from_dir(std::path::Path::new(root))?;

    reporter.stage("加载语料页面视图");
    let view = crate::update::CorpusPageView::load(std::path::Path::new(corpus))?;
    reporter.log(format!(
        "页面 {} 个 / facts {} 页",
        view.pages.len(),
        view.facts.len()
    ));

    reporter.stage("生成高引用 symbol 证据包");
    let packs = crate::update::build_symbol_evidence_packs(&atlas.index, &view, *limit);
    reporter.log(format!("生成 {} 个 symbol 证据包", packs.len()));
    if *llm {
        reporter.log(format!(
            "LLM 分批策略：每批 ≤ {} 页（0 = 单批）",
            batch_pages
        ));
    }

    let out_dir = out.unwrap_or_else(|| {
        PathBuf::from("output").join("symbol-annotate").join(
            atlas
                .build_id
                .clone()
                .unwrap_or_else(|| "unknown".to_string()),
        )
    });
    std::fs::create_dir_all(&out_dir)?;

    let packs_path = out_dir.join("symbol_packs.json");
    crate::platform::fs::write_json_atomic(&packs_path, &packs)?;

    let mut prompts = String::from("# Symbol Annotation Prompts\n\n");
    for pack in &packs {
        let symbol = match &pack.symbol {
            crate::update::SymbolRef::File { path } => path.clone(),
            crate::update::SymbolRef::Fn { file, name } => format!("{file}#{name}"),
            crate::update::SymbolRef::Const { file, name } => format!("{file}#{name}"),
            crate::update::SymbolRef::Behaviour { name } => format!("behaviours/{name}"),
        };
        let semantics = format!("{}（代码证据待补充）", symbol);
        prompts.push_str(&crate::update::render_symbol_annotation_prompt(
            pack, &semantics,
        ));
        prompts.push('\n');
    }
    let prompts_path = out_dir.join("symbol_prompts.md");
    std::fs::write(&prompts_path, &prompts)?;

    let mut summary = serde_json::json!({
        "packs": packs.len(),
        "output": out_dir,
        "prompts": prompts_path,
        "packs_file": packs_path,
    });

    let mut llm_responses: Option<Vec<crate::update::SymbolAnnotationResponse>> = None;
    if let Some(vp) = verdicts {
        reporter.stage("读取标注结果并生成覆盖报告");
        let raw = std::fs::read_to_string(vp)?;
        let responses: Vec<crate::update::SymbolAnnotationResponse> = serde_json::from_str(&raw)?;
        llm_responses = Some(responses);
    } else if *llm {
        reporter.stage("LLM 标注");
        match crate::llm::LlmConfig::from_env() {
            None => {
                reporter
                    .log("未配置 LLM__API_KEY，跳过 LLM 标注，仅生成 Prompt/证据包".to_string());
            }
            Some(config) => {
                reporter.log(format!("使用模型 {} / {}", config.model, config.base_url));
                let mut responses = Vec::new();
                let mut batches_total = 0usize;
                let mut batches_failed = 0usize;
                let mut batches_skipped_local = 0usize;
                // Raw LLM responses are kept on disk first so a malformed
                // output can be diagnosed (or repaired) after the fact.
                let raw_dir = out_dir.join("llm_raw");
                std::fs::create_dir_all(&raw_dir)?;
                let system =
                    "你是 DST Huiji Wiki 的代码符号页面影响标注助手。必须严格按用户要求输出 JSON。";
                for pack in &packs {
                    let symbol = crate::update::symbol_display(&pack.symbol);
                    let raw_stem = symbol.replace(['/', '\\', '#', ':'], "_");
                    let semantics = format!("{symbol}（代码证据待补充）");

                    // Bounded per-request work: optionally keep zero-evidence
                    // pages out of the LLM path, then split the rest into
                    // evidence-ranked batches under both caps.
                    let (send_pids, local_verdicts) =
                        crate::update::partition_no_fact_pages(pack, *skip_no_fact_pages);
                    let mut working = pack.clone();
                    working.affected_pageids = send_pids;
                    let batches = crate::update::paginate_affected_pages(
                        &working,
                        *batch_pages,
                        *batch_max_chars,
                    );
                    let total_pages: usize = batches.iter().map(Vec::len).sum();
                    reporter.log(format!(
                        "标注 {symbol}：送审 {total_pages} 页分 {} 批（≤ {} 页 / ≤ {}B 每批）",
                        batches.len(),
                        batch_pages,
                        batch_max_chars,
                    ));
                    let mut sym_verdicts = Vec::new();
                    let mut sym_local = Vec::new();
                    if !local_verdicts.is_empty() {
                        reporter.log(format!(
                            "另有 {} 个零证据页走本地判定（不消耗 LLM）",
                            local_verdicts.len()
                        ));
                        batches_skipped_local += 1;
                        sym_local.extend(local_verdicts);
                    }
                    batches_total += batches.len();

                    for (bi, batch) in batches.iter().enumerate() {
                        let prompt = crate::update::render_symbol_annotation_prompt_for_pages(
                            pack, &semantics, batch,
                        );
                        // One immediate retry per batch; a twice-failed batch
                        // is skipped and accounted for instead of sinking the
                        // whole run.
                        let mut outcome = None;
                        for attempt in 1..=2 {
                            reporter.log(format!(
                                "批次 {}/{}（{} 页）第 {attempt} 次尝试…",
                                bi + 1,
                                batches.len(),
                                batch.len(),
                            ));
                            let raw_file = raw_dir.join(format!("{raw_stem}_b{:02}.json", bi + 1));
                            let result = async {
                                let raw = config
                                    .complete_streaming(system, &prompt, |ev| match ev {
                                        crate::llm::LlmStreamEvent::FirstToken { elapsed_secs } => {
                                            reporter.log(format!(
                                                "首个输出分片已到达（{elapsed_secs}s）"
                                            ));
                                        }
                                        crate::llm::LlmStreamEvent::Tick { chars } => {
                                            if chars == 0 {
                                                reporter.log("仍在等待模型输出…".to_string());
                                            } else {
                                                reporter.log(format!("流式接收中：{chars} 字符"));
                                            }
                                        }
                                        crate::llm::LlmStreamEvent::Done {
                                            chars,
                                            elapsed_secs,
                                        } => {
                                            reporter.log(format!(
                                                "输出完成：{chars} 字符 / {elapsed_secs}s"
                                            ));
                                        }
                                    })
                                    .await?;
                                std::fs::write(&raw_file, &raw).map_err(crate::error::Error::Io)?;
                                crate::update::parse_symbol_annotation_response(&raw).map_err(|e| {
                                    crate::error::Error::Llm(format!(
                                        "输出无法解析为 JSON（原始响应已保存到 {}）：{e}",
                                        raw_file.display()
                                    ))
                                })
                            }
                            .await;
                            match result {
                                Ok(verdicts) => {
                                    outcome = Some(Ok(verdicts));
                                    break;
                                }
                                Err(e) => outcome = Some(Err(e)),
                            }
                        }
                        match outcome.expect("loop ran at least once") {
                            Ok(verdicts) => sym_verdicts.extend(verdicts),
                            Err(e) => {
                                batches_failed += 1;
                                reporter.log(format!(
                                    "批次 {}/{} 两次失败，跳过：{e}",
                                    bi + 1,
                                    batches.len()
                                ));
                            }
                        }
                    }
                    // Guardrail: every requested page must end up answered.
                    let gap_filled = crate::update::fill_missing_requested(
                        &working.affected_pageids,
                        &mut sym_verdicts,
                    );
                    if gap_filled > 0 {
                        reporter.log(format!("对账补齐 {gap_filled} 页批次输出缺失的判定"));
                    }
                    sym_verdicts.append(&mut sym_local);
                    reporter.log(format!(
                        "标注 {} 完成（{} 条 verdict）",
                        symbol,
                        sym_verdicts.len()
                    ));
                    responses.push(crate::update::SymbolAnnotationResponse {
                        symbol,
                        verdicts: sym_verdicts,
                    });
                }
                summary["llm_batches"] = serde_json::json!({
                    "total": batches_total,
                    "failed": batches_failed,
                    "max_pages_per_batch": *batch_pages,
                    "max_input_chars_per_batch": *batch_max_chars,
                    "symbols_with_local_only_pages": batches_skipped_local,
                });
                if batches_failed > 0 {
                    reporter.log(format!(
                        "警告：{batches_failed}/{batches_total} 个批次失败被跳过"
                    ));
                }
                let verdicts_path = out_dir.join("symbol_verdicts.json");
                crate::platform::fs::write_json_atomic(&verdicts_path, &responses)?;
                reporter.log(format!(
                    "LLM 标注完成 {} 个 symbol → {}",
                    responses.len(),
                    verdicts_path.display()
                ));
                summary["llm_verdicts"] = serde_json::json!(verdicts_path);
                llm_responses = Some(responses);
            }
        }
    }

    if let Some(responses) = llm_responses {
        let reports = crate::update::build_coverage_reports(&responses);
        let cov_json = out_dir.join("symbol_coverage.json");
        crate::platform::fs::write_json_atomic(&cov_json, &reports)?;
        let mut md = String::from("# Symbol Coverage 报告\n\n");
        for r in &reports {
            md.push_str(&crate::update::render_coverage_report_md(r));
        }
        let cov_md = out_dir.join("symbol_coverage.md");
        std::fs::write(&cov_md, &md)?;
        reporter.log(format!(
            "覆盖报告 {} 个 symbol → {} / {}",
            reports.len(),
            cov_json.display(),
            cov_md.display()
        ));
        summary["coverage"] = serde_json::json!({
            "symbols": reports.len(),
            "json": cov_json,
            "markdown": cov_md,
        });
    }

    reporter.log(format!("已写入 {}", out_dir.display()));
    Ok(summary)
}

async fn run_parse_po(
    input: &str,
    output: Option<PathBuf>,
    category: Option<String>,
    reporter: &dyn Reporter,
) -> Result<serde_json::Value> {
    reporter.stage("解析 PO 文件");
    let po_file = crate::parser::PoParser::parse_from_file(path_str(input)?)?;
    let entries = if let Some(cat) = category {
        po_file.filter_by_category(&cat)
    } else {
        po_file.entries.iter().collect::<Vec<_>>()
    };

    reporter.log(format!("共 {} 条目", entries.len()));

    if let Some(output_path) = output {
        crate::platform::fs::write_json_atomic(&output_path, &entries)?;
        reporter.log(format!("已写入 {} 条目到 {:?}", entries.len(), output_path));
        Ok(serde_json::json!({ "entries": entries.len(), "output": output_path }))
    } else {
        for entry in entries.iter().take(10) {
            reporter.log(format!("{:?}", entry));
        }
        reporter.log(format!("... 共 {} 条目", entries.len()));
        Ok(serde_json::json!({ "entries": entries.len() }))
    }
}

async fn run_map_names(
    input: &str,
    output: Option<PathBuf>,
    compare: Option<String>,
    merge: bool,
    version: Option<String>,
    reporter: &dyn Reporter,
) -> Result<serde_json::Value> {
    reporter.stage("解析 PO 文件");
    let po_file = crate::parser::PoParser::parse_from_file(path_str(input)?)?;
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

    reporter.log(format!("找到 {} 条 NAMES 条目", names_entries.len()));

    let converter = WikiDataConverter::new();
    let version_str = version.as_deref().unwrap_or("unknown");
    let sources = format!("Extract data from patch {}", version_str);
    let description = Some(serde_json::json!({
        "zh": "饥荒联机版的实体代码、中英文名及图片对照表"
    }));

    reporter.stage("生成维基数据");
    let wiki_data = if merge {
        let compare_path = compare
            .clone()
            .ok_or_else(|| Error::Config("--merge requires --compare".to_string()))?;
        let historical_json = std::fs::read_to_string(&compare_path)?;
        let historical_data = WikiDataConverter::parse_wiki_json(&historical_json)?;
        converter.convert_with_history(&names_entries, &sources, &historical_data, description)
    } else {
        converter.convert_to_wiki_json(&names_entries, &sources, description)
    };

    if let Some(compare_path) = &compare {
        if !merge {
            let historical_json = std::fs::read_to_string(compare_path)?;
            let historical_data = WikiDataConverter::parse_wiki_json(&historical_json)?;
            reporter.log(compare_and_report(&wiki_data, &historical_data));
        }
    }

    finish_json_output(&wiki_data, output, reporter)
}

async fn run_map_recipes(
    input: &str,
    output: Option<PathBuf>,
    compare: Option<String>,
    merge: bool,
    po_file: Option<String>,
    version: Option<String>,
    reporter: &dyn Reporter,
) -> Result<serde_json::Value> {
    reporter.stage("解析配方 Lua");
    let lua_content = std::fs::read_to_string(input)?;
    let mut parser = RecipeParser::new();
    let recipes = parser.parse(&lua_content, Some(input))?;

    reporter.log(format!("找到 {} 个配方", recipes.len()));

    let converter = if let Some(po_path) = &po_file {
        match crate::parser::PoParser::parse_from_file(po_path) {
            Ok(po_data) => {
                reporter.log(format!(
                    "载入 {} 条 PO 条目用于描述查询",
                    po_data.entries.len()
                ));
                WikiDataConverter::with_po_entries(po_data.entries.clone())
            }
            Err(e) => {
                reporter.log(format!("警告：PO 文件加载失败：{}", e));
                WikiDataConverter::new()
            }
        }
    } else {
        WikiDataConverter::new()
    };

    let version_str = version.as_deref().unwrap_or("unknown");
    let sources = format!("Extract data from patch {}", version_str);
    let description = Some(serde_json::json!({
        "zh": "饥荒联机版的合成配方列表"
    }));

    reporter.stage("生成维基数据");
    let wiki_data = if merge {
        let compare_path = compare
            .clone()
            .ok_or_else(|| Error::Config("--merge requires --compare".to_string()))?;
        let historical_json = std::fs::read_to_string(&compare_path)?;
        let historical_data = WikiDataConverter::parse_wiki_json(&historical_json)?;
        let mut data = converter.convert_recipes(&recipes, &sources, description);
        crate::models::Recipe::merge_with_history(&mut data, &historical_data);
        data
    } else {
        converter.convert_recipes(&recipes, &sources, description)
    };

    if let Some(compare_path) = &compare {
        if !merge {
            let historical_json = std::fs::read_to_string(compare_path)?;
            let historical_data = WikiDataConverter::parse_wiki_json(&historical_json)?;
            reporter.log(compare_and_report(&wiki_data, &historical_data));
        }
    }

    finish_json_output(&wiki_data, output, reporter)
}

// ---------------------------------------------------------------------------
// Corpus harvesting (wiki read-only, no game dir required)
// ---------------------------------------------------------------------------

/// `scripts-sync`: archive the live scripts tree as a snapshot and extract
/// the new `scripts.zip`. Local-only; never touches the wiki.
async fn run_scripts_sync(
    force: bool,
    dry_run: bool,
    state_path: Option<&str>,
    reporter: &dyn Reporter,
) -> Result<serde_json::Value> {
    let dst_root = crate::platform::config::dst_root_str()?;
    crate::scripts_sync::sync(
        &crate::scripts_sync::SyncParams {
            dst_root,
            state_path: state_path.map(PathBuf::from),
            force,
            dry_run,
        },
        reporter,
    )
}

/// `images-sync`: process the game image pipeline. Local-only image work;
/// afterwards refreshes `history/icon_meta.json` (names from the game PO
/// files and, unless skipped, wiki upload status via read-only queries).
async fn run_images_sync(
    force: bool,
    dry_run: bool,
    skip_wiki_status: bool,
    reporter: &dyn Reporter,
) -> Result<serde_json::Value> {
    let dst_root = crate::platform::config::dst_root_str()?;
    let out_dir = Some(crate::platform::config::ktools_out_dir());
    let mut report = crate::scripts_sync::images::run(
        &crate::scripts_sync::images::ImagesSyncParams {
            dst_root: dst_root.clone(),
            out_dir: out_dir.clone(),
            force,
            dry_run,
        },
        reporter,
    )?;

    let status = report
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    if !dry_run && status != "rollback_skipped" {
        let out = out_dir
            .clone()
            .unwrap_or_else(upload_icons::out_dir_from_env);
        match upload_icons::refresh_icon_meta(
            Path::new(&dst_root),
            &out,
            force,
            !skip_wiki_status,
            reporter,
        )
        .await
        {
            Ok(summary) => report["icon_meta"] = summary,
            Err(e) => {
                reporter.log(format!("图标元数据刷新失败：{e}"));
                report["icon_meta_error"] = serde_json::json!(e.to_string());
            }
        }
    }
    Ok(report)
}

/// `anim-sync`: archive current `data/anim` into `ANIM__OUT_DIR` after a game
/// update. Local-only; never touches the wiki.
async fn run_anim_sync(
    force: bool,
    dry_run: bool,
    label: Option<String>,
    out: Option<PathBuf>,
    reporter: &dyn Reporter,
) -> Result<serde_json::Value> {
    let dst_root = crate::platform::config::dst_root_str()?;
    crate::scripts_sync::anim::run_sync(
        &crate::scripts_sync::anim::AnimSyncParams {
            dst_root,
            out_dir: out,
            label,
            force,
            dry_run,
        },
        reporter,
    )
}

/// `anim-diff`: compare two animation directories. Local-only.
async fn run_anim_diff(
    old_dir: PathBuf,
    new_dir: PathBuf,
    zip: Option<String>,
    reporter: &dyn Reporter,
) -> Result<serde_json::Value> {
    crate::scripts_sync::anim::run_diff(
        &crate::scripts_sync::anim::AnimDiffParams {
            old_dir,
            new_dir,
            only: zip,
            parse_all: false,
        },
        reporter,
    )
}

/// `anim-index`: build a prefab ↔ animation-file association index.
/// Local-only; no wiki traffic.
async fn run_anim_index(
    scripts: &str,
    anim: Option<PathBuf>,
    out: Option<PathBuf>,
    reporter: &dyn Reporter,
) -> Result<serde_json::Value> {
    crate::scripts_sync::anim::index::run_index(
        &crate::scripts_sync::anim::index::AnimIndexParams {
            scripts_root: PathBuf::from(scripts),
            anim_root: anim,
            out,
        },
        reporter,
    )
}

/// `skin-index`: parse `prefabs/skinprefabs.lua` and pair skin builds with
/// `data/anim/dynamic` zip/dyn packages. Local-only; no wiki traffic.
fn run_skin_index(
    scripts: Option<&str>,
    anim: Option<PathBuf>,
    out: Option<PathBuf>,
    reporter: &dyn Reporter,
) -> Result<serde_json::Value> {
    let scripts_root = match scripts.map(PathBuf::from) {
        Some(p) => p,
        None => crate::scripts_sync::anim::skin_index::default_scripts_root().ok_or_else(|| {
            Error::EnvVarNotFound(
                "scripts root not provided and DST__ROOT/data/databundles/scripts not found"
                    .to_string(),
            )
        })?,
    };
    crate::scripts_sync::anim::skin_index::run_skin_index(
        &crate::scripts_sync::anim::skin_index::SkinIndexParams {
            scripts_root,
            anim_root: anim,
            out,
        },
        reporter,
    )
}

async fn run_corpus_fetch(
    full: bool,
    rc: bool,
    dir: Option<&str>,
    reporter: &dyn Reporter,
    mode: WriteMode,
) -> Result<serde_json::Value> {
    // 唯一允许匿名读的作业（读旧缓存可接受）；其余作业读也要求登录。
    // Login is deliberately skipped — this job never edits.
    let client = WikiClient::from_env_readonly()?;
    let base_dir = PathBuf::from(dir.unwrap_or("wikis"));
    if rc {
        return crate::corpus::rc::sync_rc(&client, &base_dir, mode == WriteMode::DryRun, reporter)
            .await;
    }
    crate::corpus::sync(
        &client,
        &base_dir,
        full,
        mode == WriteMode::DryRun,
        reporter,
    )
    .await
}

// ---------------------------------------------------------------------------
// Game-dir maintenance commands (require DST__ROOT, touch the wiki)
// ---------------------------------------------------------------------------

/// Rebuilds derived corpus indexes from the local tree. Purely local I/O —
/// no client, no login, `DST__ROOT` not required.
async fn run_corpus_index(
    dir: Option<&str>,
    join: Option<&str>,
    reporter: &dyn Reporter,
    mode: WriteMode,
) -> Result<serde_json::Value> {
    let base_dir = PathBuf::from(dir.unwrap_or("wikis"));
    crate::corpus::build_indexes(
        &base_dir,
        join.map(PathBuf::from).as_deref(),
        mode == WriteMode::DryRun,
        reporter,
    )
    .await
}

async fn run_maintain_item_table(
    output: Option<PathBuf>,
    snapshot: Option<String>,
    reporter: &dyn Reporter,
    mode: WriteMode,
) -> Result<serde_json::Value> {
    let mut ctx = make_ctx(&snapshot)?;
    reporter.log(format!("DST 版本: {}", ctx.version));

    reporter.stage("登录维基");
    ctx.wiki().login().await?;

    reporter.stage("解析 chinese_s.po");
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

    reporter.log(format!("找到 {} 条 NAMES 条目", names_entries.len()));

    let converter = WikiDataConverter::new();

    reporter.stage("拉取维基历史数据");
    let page_title = "Data:ItemTable.tabx";
    let historical_data = match ctx.wiki().get_json_data(page_title).await {
        Ok(historical_json) => Some(WikiDataConverter::parse_wiki_json(
            &historical_json.to_string(),
        )?),
        Err(e) => {
            reporter.log(format!("警告：获取历史数据失败：{}", e));
            None
        }
    };

    let sources = ctx.sources();
    let description = Some(serde_json::json!({
        "zh": "饥荒联机版的实体代码、中英文名及图片对照表"
    }));
    let mut wiki_data = converter.convert_to_wiki_json(&names_entries, &sources, description);

    let mut merged = false;
    if let Some(ref historical) = historical_data {
        PoEntry::merge_with_history(&mut wiki_data, historical);
        reporter.log(compare_and_report(&wiki_data, historical));
        merged = true;
    }

    let wctx = WriteCtx {
        client: ctx.wiki(),
        reporter,
        mode,
    };
    let summary = output_json_result_with_update(&wctx, page_title, &wiki_data, output).await?;
    Ok(serde_json::json!({ "merged_with_history": merged, "summary": summary }))
}

async fn run_maintain_dst_recipes(
    output: Option<PathBuf>,
    snapshot: Option<String>,
    reporter: &dyn Reporter,
    mode: WriteMode,
) -> Result<serde_json::Value> {
    let mut ctx = make_ctx(&snapshot)?;
    reporter.log(format!("DST 版本: {}", ctx.version));

    reporter.stage("登录维基");
    ctx.wiki().login().await?;

    reporter.stage("解析 recipes.lua");
    let recipes_string = ctx.read_script_file("scripts/recipes.lua")?;

    let mut parser = RecipeParser::new();
    let recipes = parser.parse(&recipes_string, Some("scripts/recipes.lua"))?;
    reporter.log(format!("共 {} 个配方", recipes.len()));

    reporter.stage("对比维基科技数据");
    let mut tech_report = TechReport::from_recipes(&recipes);
    match ctx.wiki().get_page("模块:RenderRecsByIngre/Data").await {
        Ok(page) => {
            if let Some(content) = &page.content {
                tech_report.compare_with_wiki(content);
                reporter.log(tech_report.generate_report());
            } else {
                reporter.log("警告：维基页面无内容".to_string());
            }
        }
        Err(e) => {
            reporter.log(format!("警告：获取科技数据失败：{}", e));
        }
    }

    reporter.stage("解析 chinese_s.po（描述查询）");
    let po_file = ctx.parse_po_file("scripts/languages/chinese_s.po")?;
    reporter.log(format!("载入 {} 条 PO 条目", po_file.entries.len()));

    let converter = WikiDataConverter::with_po_entries(po_file.entries.clone());

    reporter.stage("拉取维基历史数据");
    let page_title = "Data:DSTRecipes.tabx";
    let historical_data = match ctx.wiki().get_json_data(page_title).await {
        Ok(historical_json) => Some(WikiDataConverter::parse_wiki_json(
            &historical_json.to_string(),
        )?),
        Err(e) => {
            reporter.log(format!("警告：获取历史数据失败：{}", e));
            None
        }
    };

    let sources = ctx.sources();
    let description = Some(serde_json::json!({
        "zh": "饥荒联机版的合成配方列表"
    }));
    let mut wiki_data = converter.convert_recipes(&recipes, &sources, description);

    let mut merged = false;
    if let Some(ref historical) = historical_data {
        crate::models::Recipe::merge_with_history(&mut wiki_data, historical);
        reporter.log(compare_and_report(&wiki_data, historical));
        merged = true;
    }

    let wctx = WriteCtx {
        client: ctx.wiki(),
        reporter,
        mode,
    };
    let summary = output_json_result_with_update(&wctx, page_title, &wiki_data, output).await?;
    Ok(
        serde_json::json!({ "merged_with_history": merged, "recipe_count": recipes.len(), "summary": summary }),
    )
}

async fn run_maintain_copyclip(
    r#type: Option<&str>,
    output: Option<PathBuf>,
    snapshot: Option<String>,
    reporter: &dyn Reporter,
    mode: WriteMode,
) -> Result<serde_json::Value> {
    let mut ctx = make_ctx(&snapshot)?;
    reporter.log(format!("DST 版本: {}", ctx.version));

    reporter.stage("登录维基");
    ctx.wiki().login().await?;

    let types_to_run: Vec<String> = if let Some(t) = r#type {
        vec![t.to_lowercase()]
    } else {
        vec![
            "recipe_builder_tag_lookup".to_string(),
            "tech".to_string(),
            "crafting_filters".to_string(),
            "crafting_names".to_string(),
        ]
    };

    let mut results = serde_json::Map::new();
    for t in types_to_run {
        reporter.stage(&format!("运行 {}", t));
        let summary = match t.as_str() {
            "recipe_builder_tag_lookup" | "rbtl" => {
                maintain_recipe_builder_tag_lookup(&mut ctx, output.clone(), reporter, mode).await?
            }
            "tech" => maintain_tech(&mut ctx, output.clone(), reporter, mode).await?,
            "crafting_filters" | "filters" => {
                maintain_crafting_filters(&mut ctx, output.clone(), reporter, mode).await?
            }
            "crafting_names" | "names" => {
                maintain_crafting_names(&mut ctx, output.clone(), reporter, mode).await?
            }
            _ => {
                return Err(Error::Config(format!(
                    "未知的 copyclip 类型: {}。有效类型: recipe_builder_tag_lookup (rbtl), tech, crafting_filters (filters), crafting_names (names)",
                    t
                )));
            }
        };
        results.insert(t, summary);
    }

    Ok(serde_json::Value::Object(results))
}

async fn maintain_recipe_builder_tag_lookup(
    ctx: &mut DstContext,
    output: Option<PathBuf>,
    reporter: &dyn Reporter,
    mode: WriteMode,
) -> Result<serde_json::Value> {
    let debugcommands_string = ctx.read_script_file("scripts/debugcommands.lua")?;

    reporter.log("获取维基页面内容...".to_string());
    let page_title = "模块:Constants/RecipeBuilderTagLookup";
    let page = ctx.wiki().get_page(page_title).await?;

    let target_content = page
        .content
        .ok_or_else(|| Error::WikiApi("Wiki page has no content".to_string()))?;

    reporter.log("从 debugcommands.lua 提取 RECIPE_BUILDER_TAG_LOOKUP...".to_string());
    let result = crate::copyclip::process_copyclip(
        &debugcommands_string,
        "RECIPE_BUILDER_TAG_LOOKUP",
        &target_content,
    )?;

    reporter.log(format!(
        "CopyClip 完成！提取内容长度: {} 字节",
        result.extracted_content.len()
    ));

    let wctx = WriteCtx {
        client: ctx.wiki(),
        reporter,
        mode,
    };

    output_copyclip_result_with_update(
        &wctx,
        page_title,
        &target_content,
        &result.updated_content,
        output,
        page.last_rev_timestamp.clone(),
    )
    .await
}

async fn maintain_tech(
    ctx: &mut DstContext,
    output: Option<PathBuf>,
    reporter: &dyn Reporter,
    mode: WriteMode,
) -> Result<serde_json::Value> {
    let constants_string = ctx.read_script_file("scripts/constants.lua")?;

    reporter.log("获取维基页面内容...".to_string());
    let page_title = "模块:Constants/Tech";
    let page = ctx.wiki().get_page(page_title).await?;

    let target_content = page
        .content
        .ok_or_else(|| Error::WikiApi("Wiki page has no content".to_string()))?;

    reporter.log("从 constants.lua 提取 TECH...".to_string());
    let result = crate::copyclip::process_copyclip(&constants_string, "TECH", &target_content)?;

    reporter.log(format!(
        "CopyClip 完成！提取内容长度: {} 字节",
        result.extracted_content.len()
    ));

    let wctx = WriteCtx {
        client: ctx.wiki(),
        reporter,
        mode,
    };

    output_copyclip_result_with_update(
        &wctx,
        page_title,
        &target_content,
        &result.updated_content,
        output,
        page.last_rev_timestamp.clone(),
    )
    .await
}

async fn maintain_crafting_filters(
    ctx: &mut DstContext,
    output: Option<PathBuf>,
    reporter: &dyn Reporter,
    mode: WriteMode,
) -> Result<serde_json::Value> {
    let filter_string = ctx.read_script_file("scripts/recipes_filter.lua")?;

    reporter.log("获取维基页面内容...".to_string());
    let page_title = "模块:Constants/CraftingFilters";
    let page = ctx.wiki().get_page(page_title).await?;

    let target_content = page
        .content
        .ok_or_else(|| Error::WikiApi("Wiki page has no content".to_string()))?;

    reporter.log("从 recipes_filter.lua 提取 CRAFTING_FILTERS 配方列表...".to_string());
    let field_location = extract_field_assignment_range(
        &filter_string,
        "CRAFTING_FILTERS.CHARACTER.recipes",
        "CRAFTING_FILTERS.DECOR.recipes",
    )?;

    reporter.log(format!(
        "提取内容长度: {} 字节",
        field_location.content.len()
    ));

    let marker_range = crate::copyclip::CopyClipProcessor::find_marker_range(&target_content)?;
    let updated_content = crate::copyclip::CopyClipProcessor::replace_between_markers(
        &target_content,
        &marker_range,
        &field_location.content,
    );

    reporter.log("CopyClip 完成！".to_string());

    let wctx = WriteCtx {
        client: ctx.wiki(),
        reporter,
        mode,
    };

    output_copyclip_result_with_update(
        &wctx,
        page_title,
        &target_content,
        &updated_content,
        output,
        page.last_rev_timestamp.clone(),
    )
    .await
}

/// 读取游戏侧四张表并派生 CraftingNames 需要补的制作站别名。
///
/// 派生链：`constants.lua TECH` → 科技树键 →（`tuning.lua`
/// `PROTOTYPER_TREES` 反查）→ 条目名（即 PO 站筛键，如
/// `CARNIVALGAME_GOLFGAME`）。任一步读文件/解析失败即返回 `Err`，
/// 由调用方决定降级。
fn derive_crafting_station_aliases(
    ctx: &mut DstContext,
    known_station_keys: &BTreeSet<String>,
) -> Result<StationAliasReport> {
    let constants = ctx.read_script_file("scripts/constants.lua")?;
    let tuning = ctx.read_script_file("scripts/tuning.lua")?;
    let recipes_source = ctx.read_script_file("scripts/recipes.lua")?;
    let filters_source = ctx.read_script_file("scripts/recipes_filter.lua")?;

    let tech_constants = parse_tech_constants(&constants)?;
    let prototyper_trees = parse_prototyper_trees(&tuning)?;
    let filter_lists = parse_crafting_filter_lists(&filters_source)?;

    let mut parser = RecipeParser::new();
    let recipes = parser.parse(&recipes_source, Some("scripts/recipes.lua"))?;
    let recipe_techs: BTreeMap<String, String> = recipes
        .iter()
        .map(|r| (r.name.clone(), r.tech.clone()))
        .collect();

    let station_recipes = filter_lists
        .get("CRAFTING_STATION")
        .cloned()
        .unwrap_or_default();
    let fallback = BTreeMap::new();

    Ok(derive_station_aliases(&StationAliasInputs {
        tech_constants: &tech_constants,
        prototyper_trees: &prototyper_trees,
        known_station_keys,
        station_recipes: &station_recipes,
        recipe_techs: &recipe_techs,
        fallback: &fallback,
    }))
}

async fn maintain_crafting_names(
    ctx: &mut DstContext,
    output: Option<PathBuf>,
    reporter: &dyn Reporter,
    mode: WriteMode,
) -> Result<serde_json::Value> {
    let po_file = ctx.parse_po_file("scripts/languages/chinese_s.po")?;

    let station_prefix = "STRINGS.UI.CRAFTING_STATION_FILTERS.";
    let filter_prefix = "STRINGS.UI.CRAFTING_FILTERS.";

    let mut crafting_stations: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    let mut craftings: BTreeMap<String, serde_json::Value> = BTreeMap::new();

    for entry in &po_file.entries {
        if let Some(ref entry_ctx) = entry.msgctxt {
            if let Some(key) = entry_ctx.strip_prefix(station_prefix) {
                crafting_stations.insert(
                    key.to_string(),
                    serde_json::json!({
                        "station_en": entry.msgid.clone(),
                        "station_cn": entry.msgstr.clone(),
                    }),
                );
            } else if let Some(key) = entry_ctx.strip_prefix(filter_prefix) {
                craftings.insert(
                    key.to_string(),
                    serde_json::json!({
                        "station_en": entry.msgid.clone(),
                        "station_cn": entry.msgstr.clone(),
                    }),
                );
            }
        }
    }

    // D2：补制作站别名。游戏侧科技树键与 PO 站筛键不同名时（如
    // CARNIVAL_GOLFPROPS ↔ CARNIVALGAME_GOLFGAME），wiki 端
    // `模块:DSTRecipe.get_recipe_filter_cn` 会 assert，必须在
    // crafting_stations 里保留一个别名键。派生失败只警告，不阻断生成。
    let known_station_keys: BTreeSet<String> = crafting_stations.keys().cloned().collect();
    let mut alias_added = Vec::new();
    match derive_crafting_station_aliases(ctx, &known_station_keys) {
        Ok(report) => {
            for alias in &report.aliases {
                if crafting_stations.contains_key(&alias.tree_key) {
                    continue;
                }
                if let Some(target) = crafting_stations.get(&alias.filter_key).cloned() {
                    crafting_stations.insert(alias.tree_key.clone(), target);
                    alias_added.push(format!("{}→{}", alias.tree_key, alias.filter_key));
                }
            }
            if !alias_added.is_empty() {
                reporter.log(format!(
                    "已补 {} 条制作站别名：{}",
                    alias_added.len(),
                    alias_added.join(", ")
                ));
            }
            for u in &report.unresolved {
                let total = u.sample_recipes.len();
                let samples = u.sample_recipes.iter().take(5).cloned().collect::<Vec<_>>();
                let suffix = if total > samples.len() {
                    format!(" …（共 {} 条）", total)
                } else {
                    String::new()
                };
                reporter.log(format!(
                    "警告：制作站别名未决 {}（{:?}），样例配方：{}{}",
                    u.tree_key,
                    u.reason,
                    samples.join(", "),
                    suffix
                ));
            }
        }
        Err(e) => reporter.log(format!("警告：制作站别名派生失败：{e}")),
    }

    let stations_len = crafting_stations.len();
    let craftings_len = craftings.len();

    let crafting_names = serde_json::json!({
        "crafting_stations": crafting_stations,
        "craftings": craftings
    });

    let json_content = serde_json::to_string_pretty(&crafting_names)?;
    reporter.log(format!(
        "找到 {} 个制作站点和 {} 个制作分类",
        stations_len, craftings_len
    ));

    reporter.log("获取维基页面内容...".to_string());
    let page_title = "模块:Constants/CraftingNames";
    let page = ctx.wiki().get_page(page_title).await?;

    let target_content = page
        .content
        .ok_or_else(|| Error::WikiApi("Wiki page has no content".to_string()))?;

    reporter.log("定位 [[ 与 ]] 标记...".to_string());
    let start_marker = "[[";
    let end_marker = "]]";

    let start_pos = target_content
        .find(start_marker)
        .ok_or_else(|| Error::ParseError("'[[' marker not found".to_string()))?;
    let end_pos = target_content
        .rfind(end_marker)
        .ok_or_else(|| Error::ParseError("']]' marker not found".to_string()))?;

    if start_pos >= end_pos {
        return Err(Error::ParseError(
            "'[[' must appear before ']]'".to_string(),
        ));
    }

    let updated_content = format!(
        "{}{}\n{}",
        &target_content[..start_pos + start_marker.len()],
        json_content,
        &target_content[end_pos..]
    );

    reporter.log("CopyClip 完成！".to_string());

    let wctx = WriteCtx {
        client: ctx.wiki(),
        reporter,
        mode,
    };

    output_copyclip_result_with_update(
        &wctx,
        page_title,
        &target_content,
        &updated_content,
        output,
        page.last_rev_timestamp.clone(),
    )
    .await
}

// ---------------------------------------------------------------------------
// Shared output helpers (wiki-write confirmation flows through the reporter)
// ---------------------------------------------------------------------------

fn finish_json_output(
    wiki_data: &crate::mapping::WikiJsonData,
    output: Option<PathBuf>,
    reporter: &dyn Reporter,
) -> Result<serde_json::Value> {
    if let Some(output_path) = output {
        let json = WikiDataConverter::to_json_string(wiki_data)?;
        crate::platform::fs::write_text_atomic(&output_path, &json)?;
        reporter.log(format!(
            "已写入 {} 条记录到 {:?}",
            wiki_data.data.len(),
            output_path
        ));
        return Ok(serde_json::json!({ "records": wiki_data.data.len(), "output": output_path }));
    }

    reporter.log(format!("共 {} 条记录", wiki_data.data.len()));
    Ok(serde_json::json!({ "records": wiki_data.data.len() }))
}

/// Shared context for the wiki-write helper paths, keeping their argument
/// lists small.
struct WriteCtx<'a> {
    client: &'a WikiClient,
    reporter: &'a dyn Reporter,
    mode: WriteMode,
}

async fn output_json_result_with_update(
    ctx: &WriteCtx<'_>,
    page_title: &str,
    wiki_data: &crate::mapping::WikiJsonData,
    output: Option<PathBuf>,
) -> Result<serde_json::Value> {
    let client = ctx.client;
    let reporter = ctx.reporter;
    let mode = ctx.mode;
    let new_json = WikiDataConverter::to_json_string(wiki_data)?;

    if let Some(output_path) = output {
        std::fs::write(&output_path, &new_json)?;
        reporter.log(format!(
            "已写入 {} 条记录到 {:?}",
            wiki_data.data.len(),
            output_path
        ));
    }

    // Fetch the current revision once: the content feeds the comparison and
    // its timestamp becomes `basetimestamp` for a conflict-safe edit.
    let page = match client.get_page(page_title).await {
        Ok(p) => p,
        Err(e) => {
            reporter.log(format!("警告：获取维基当前数据失败：{}", e));
            return Ok(
                serde_json::json!({ "status": "fetch_failed", "records": wiki_data.data.len() }),
            );
        }
    };

    let historical_content = match page.content {
        Some(c) => c,
        None => {
            reporter.log(format!("警告：页面 '{}' 无内容", page_title));
            return Ok(
                serde_json::json!({ "status": "fetch_failed", "records": wiki_data.data.len() }),
            );
        }
    };
    // Round-trip through serde so both sides are compared with identical
    // pretty-printing, independent of how the page was last saved.
    let historical_json: String = match serde_json::from_str::<serde_json::Value>(
        historical_content.trim(),
    ) {
        Ok(v) => serde_json::to_string_pretty(&v)?,
        Err(e) => {
            reporter.log(format!("警告：解析维基历史数据失败：{}", e));
            return Ok(
                serde_json::json!({ "status": "fetch_failed", "records": wiki_data.data.len() }),
            );
        }
    };

    let writer = wiki_write::WikiWriter::new(client, reporter, mode);
    let edit = wiki_write::PageEdit {
        title: page_title,
        new_content: &new_json,
        old_content: Some(&historical_json),
        basetimestamp: page.last_rev_timestamp.as_deref(),
        preserve_whitespace: false,
        summary: "Update via dst-huiji-wiki tool",
    };
    let outcome = writer
        .propose(&edit, new_json.trim() != historical_json.trim())
        .await?;
    Ok(serde_json::json!(
        { "status": outcome.status, "page": page_title, "newrevid": outcome.newrevid }))
}

async fn output_copyclip_result_with_update(
    ctx: &WriteCtx<'_>,
    page_title: &str,
    target_content: &str,
    updated_content: &str,
    output: Option<PathBuf>,
    base_timestamp: Option<String>,
) -> Result<serde_json::Value> {
    if target_content == updated_content {
        ctx.reporter.log("未检测到变化。".to_string());
        return Ok(serde_json::json!({ "status": "no_changes" }));
    }

    if let Some(output_path) = output {
        crate::platform::fs::write_text_atomic(&output_path, updated_content)?;
        ctx.reporter
            .log(format!("已写入更新内容到 {:?}", output_path));
    }

    let writer = wiki_write::WikiWriter::new(ctx.client, ctx.reporter, ctx.mode);
    let edit = wiki_write::PageEdit {
        title: page_title,
        new_content: updated_content,
        old_content: Some(target_content),
        basetimestamp: base_timestamp.as_deref(),
        preserve_whitespace: true,
        summary: "Update via dst-huiji-wiki tool",
    };
    let outcome = writer.propose(&edit, true).await?;
    Ok(serde_json::json!(
        { "status": outcome.status, "page": page_title, "newrevid": outcome.newrevid }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_kind_names() {
        let jobs = [
            JobKind::ParsePo {
                input: "a.po".to_string(),
                output: None,
                category: None,
            },
            JobKind::MapNames {
                input: "a.po".to_string(),
                output: None,
                compare: None,
                merge: false,
                version: None,
            },
            JobKind::MaintainItemTable {
                output: None,
                snapshot: Some("scripts_202605291134".to_string()),
            },
        ];
        assert_eq!(jobs[0].name(), "parse-po");
        assert_eq!(jobs[1].name(), "map-names");
        assert_eq!(jobs[2].name(), "maintain-item-table");

        assert!(!jobs[0].touches_wiki());
        assert!(jobs[2].touches_wiki());
    }

    #[test]
    fn test_job_kind_serde_roundtrip() {
        let kind = JobKind::MaintainCopyClip {
            r#type: Some("tech".to_string()),
            output: None,
            snapshot: None,
        };
        let json = serde_json::to_string(&kind).unwrap();
        // Tagged representation keeps the variant name stable for the WebUI API.
        assert!(json.contains("\"kind\":\"maintain_copy_clip\""), "{}", json);
        let back: JobKind = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name(), kind.name());

        // Old payloads without new optional fields must keep deserializing.
        let legacy = r#"{"kind":"maintain_item_table"}"#;
        let back: JobKind = serde_json::from_str(legacy).unwrap();
        match back {
            JobKind::MaintainItemTable { output, snapshot } => {
                assert!(output.is_none());
                assert!(snapshot.is_none());
            }
            _ => panic!("wrong variant"),
        }
    }

    /// 契约：`name()` 全集唯一，且与 serde tag 一一对应（kebab ↔ snake）。
    /// 防止再加变体时 CLI/JobKind/serde 三处名称继续漂移。
    #[test]
    fn contract_job_names_unique_and_aligned_with_serde_tags() {
        let jobs = JobKind::all_variants();
        assert_eq!(
            jobs.len(),
            32,
            "新增 JobKind 变体后请同步契约测试与前端 JOB_DEFS"
        );
        let mut names: Vec<&str> = jobs.iter().map(|j| j.name()).collect();
        names.sort_unstable();
        let count = names.len();
        names.dedup();
        assert_eq!(names.len(), count, "JobKind::name() 存在重复：{names:?}");
        for j in &jobs {
            let tag = serde_json::to_value(j).unwrap()["kind"]
                .as_str()
                .unwrap()
                .to_string();
            assert_eq!(
                tag,
                j.name().replace('-', "_"),
                "serde tag `{tag}` 与 name() `{}` 不一致",
                j.name()
            );
        }
    }

    /// 契约：`touches_wiki()` 的集合固定为已知可写 job；新增可写变体必须
    /// 显式更新本测试（并同步前端 JOB_DEFS 的 wiki 标记与确认流程）。
    #[test]
    fn contract_touches_wiki_is_the_expected_set() {
        let expected: std::collections::BTreeSet<&str> = [
            "maintain-item-table",
            "maintain-dst-recipes",
            "maintain-copy-clip",
            "maintain-strings",
            "maintain-wikitext",
            "skilltree-wiki",
            "upload-image",
            "upload-icons",
        ]
        .into_iter()
        .collect();
        let actual: std::collections::BTreeSet<&str> = JobKind::all_variants()
            .iter()
            .filter(|j| j.touches_wiki())
            .map(|j| j.name())
            .collect();
        assert_eq!(actual, expected, "touches_wiki 集合漂移");
    }

    /// 契约：前端 JOB_DEFS 的键必须是合法的 JobKind serde tag，
    /// 保证 WebUI 提交的每个任务都能被 `/api/jobs` 反序列化。
    #[test]
    fn contract_frontend_job_defs_keys_are_valid_serde_tags() {
        const APP_JS: &str = include_str!("../web/assets/app.js");
        let start = APP_JS
            .find("const JOB_DEFS = {")
            .expect("app.js 缺少 JOB_DEFS 定义");
        let block_end = start + APP_JS[start..].find("\n};").expect("JOB_DEFS 块未闭合");
        let block = &APP_JS[start..block_end];

        let tags: std::collections::BTreeSet<String> = JobKind::all_variants()
            .iter()
            .map(|j| {
                serde_json::to_value(j).unwrap()["kind"]
                    .as_str()
                    .unwrap()
                    .to_string()
            })
            .collect();

        let mut keys = Vec::new();
        for line in block.lines() {
            let Some(rest) = line.strip_prefix("  ") else {
                continue;
            };
            if rest.starts_with(' ') {
                continue; // 嵌套字段行
            }
            let Some(idx) = rest.find(": {") else {
                continue;
            };
            let key = &rest[..idx];
            if !key.is_empty()
                && key
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            {
                keys.push(key.to_string());
            }
        }
        assert!(!keys.is_empty(), "未能从 app.js 解析出 JOB_DEFS 键");
        for k in keys {
            assert!(
                tags.contains(&k),
                "前端 JOB_DEFS 键 `{k}` 不是合法的 JobKind serde tag"
            );
        }
    }

    #[test]
    fn test_decide_write_table() {
        use crate::platform::progress::{decide_write, WriteDecision};
        use WriteMode::{AutoConfirm, DryRun, Interactive};

        // DryRun never writes, regardless of confirmation.
        assert_eq!(decide_write(DryRun, true), WriteDecision::Skip("dry_run"));
        assert_eq!(decide_write(DryRun, false), WriteDecision::Skip("dry_run"));

        // AutoConfirm always applies.
        assert_eq!(decide_write(AutoConfirm, false), WriteDecision::Apply);
        assert_eq!(decide_write(AutoConfirm, true), WriteDecision::Apply);

        // Interactive follows the reporter's answer.
        assert_eq!(decide_write(Interactive, true), WriteDecision::Apply);
        assert_eq!(
            decide_write(Interactive, false),
            WriteDecision::Skip("declined")
        );
    }

    #[test]
    fn test_write_mode_name_and_default() {
        assert_eq!(WriteMode::default(), WriteMode::Interactive);
        assert_eq!(WriteMode::Interactive.name(), "interactive");
        assert_eq!(WriteMode::AutoConfirm.name(), "auto_confirm");
        assert_eq!(WriteMode::DryRun.name(), "dry_run");
    }
}
