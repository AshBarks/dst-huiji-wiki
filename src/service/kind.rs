//! `JobKind`：全部作业的参数变体与派生方法。
//!
//! 参数 schema/canonical 名等声明源在 [`super::job_spec`]；本文件只保留
//! serde 变体定义、serde 默认值函数与按表派生的方法。

use serde::{Deserialize, Serialize};

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
    super::strings_wiki::DEFAULT_BUCKETS
}

impl JobKind {
    /// 对应的 [`JobSpec`] 声明（canonical name/wiki_access/resources/params
    /// 的唯一来源）。
    pub fn spec(&self) -> &'static super::job_spec::JobSpec {
        match self {
            JobKind::ParsePo { .. } => &super::job_spec::JOB_SPECS[0],
            JobKind::MapNames { .. } => &super::job_spec::JOB_SPECS[1],
            JobKind::MapRecipes { .. } => &super::job_spec::JOB_SPECS[2],
            JobKind::MaintainItemTable { .. } => &super::job_spec::JOB_SPECS[3],
            JobKind::MaintainDstRecipes { .. } => &super::job_spec::JOB_SPECS[4],
            JobKind::MaintainCopyClip { .. } => &super::job_spec::JOB_SPECS[5],
            JobKind::MaintainTemplateCheck { .. } => &super::job_spec::JOB_SPECS[6],
            JobKind::MaintainStrings { .. } => &super::job_spec::JOB_SPECS[7],
            JobKind::SkilltreeWiki { .. } => &super::job_spec::JOB_SPECS[8],
            JobKind::SkilltreeExport { .. } => &super::job_spec::JOB_SPECS[9],
            JobKind::UploadImage { .. } => &super::job_spec::JOB_SPECS[10],
            JobKind::PrefabOverrides { .. } => &super::job_spec::JOB_SPECS[11],
            JobKind::PrefabOverridesDir { .. } => &super::job_spec::JOB_SPECS[12],
            JobKind::PrefabOverridesAudit { .. } => &super::job_spec::JOB_SPECS[13],
            JobKind::MaintainPrefabOverrides { .. } => &super::job_spec::JOB_SPECS[14],
            JobKind::ScriptsSync { .. } => &super::job_spec::JOB_SPECS[15],
            JobKind::ImagesSync { .. } => &super::job_spec::JOB_SPECS[16],
            JobKind::UploadIcons { .. } => &super::job_spec::JOB_SPECS[17],
            JobKind::AnimSync { .. } => &super::job_spec::JOB_SPECS[18],
            JobKind::AnimDiff { .. } => &super::job_spec::JOB_SPECS[19],
            JobKind::AnimIndex { .. } => &super::job_spec::JOB_SPECS[20],
            JobKind::SkinIndex { .. } => &super::job_spec::JOB_SPECS[21],
            JobKind::UpdateScan { .. } => &super::job_spec::JOB_SPECS[22],
            JobKind::UpdateIndex { .. } => &super::job_spec::JOB_SPECS[23],
            JobKind::CorpusFetch { .. } => &super::job_spec::JOB_SPECS[24],
            JobKind::CorpusIndex { .. } => &super::job_spec::JOB_SPECS[25],
            JobKind::SymbolAnnotate { .. } => &super::job_spec::JOB_SPECS[26],
            JobKind::KnowledgeScanSymbols { .. } => &super::job_spec::JOB_SPECS[27],
            JobKind::PageAssist { .. } => &super::job_spec::JOB_SPECS[28],
            JobKind::KnowledgeSync { .. } => &super::job_spec::JOB_SPECS[29],
            JobKind::KnowledgeScanWiki { .. } => &super::job_spec::JOB_SPECS[30],
            JobKind::MaintainWikitext { .. } => &super::job_spec::JOB_SPECS[31],
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
