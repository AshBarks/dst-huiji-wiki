# src/ AGENTS.md

**Scope**: All library code. Binary entrypoint is a thin shell.

## OVERVIEW
16 public modules (lib.rs) + 1 binary-only module (main.rs:commands). Standalone files at root: `context.rs`, `error.rs`, `utils.rs`. Directory modules: `commands/`, `parser/`, `models/`, `mapping/`, `wiki/`, `copyclip/`, `corpus/`, `knowledge/`, `llm/`, `update/`, `scripts_sync/`, `service/`, `platform/`, `wikitext/`.

## STRUCTURE
```
src/
├── main.rs               # bin: mod commands + dispatch to commands::run()
├── lib.rs                # 10 pub mod declarations + pub use of key types
├── commands/             # binary-only (NOT in lib.rs)
│   └── mod.rs            # clap builder 从 JOB_SPECS + CLI_EXT 生成子命令；job_from_matches 构造 JobKind；run() 分发
├── parser/               # game data parsers
│   ├── mod.rs
│   ├── lua.rs            # LuaParser (variable/field location)
│   ├── po.rs             # PoParser (nom-based PO)
│   ├── recipe.rs         # RecipeParser (full_moon AST, matches Recipe2 calls)
│   ├── skilltree.rs      # skill tree extractor (pos/connects with constant folding)
│   └── prefab_override/  # PrefabOverrideParser(parser.rs/types.rs/control_flow.rs)
├── models/               # data model structs with serde derives
│   ├── mod.rs
│   ├── po.rs             # PoEntry, PoFile
│   ├── recipe/           # Recipe + sub-models (ingredient, options, prototyper, context)
│   └── tech_report.rs    # TechReport
├── mapping/              # data to wiki mapping framework
│   ├── mod.rs
│   ├── mapper.rs         # WikiMapper trait (FieldMapping + MergeStrategy)
│   ├── converter.rs      # WikiDataConverter orchestration + 字段名对齐 diff/merge
│   ├── schema.rs         # wiki JSON schema types
│   └── mappers/          # concrete impls (PoEntryMapper, RecipeMapper)
├── platform/             # 业务无关基础设施（features→platform，platform 不依赖 service）
│   ├── mod.rs            # re-export progress
│   ├── progress.rs       # Reporter/StdoutReporter/CaptureReporter + WriteMode/decide_write
│   ├── config.rs         # DST__ROOT/KTOOLS__OUT_DIR/ANIM__OUT_DIR/ICON__TITLE_OVERRIDES 集中解析
│   ├── fs.rs             # write_text_atomic/write_json_atomic/ensure_parent
│   └── game_source.rs    # GameSource: snapshot(严格)→live→scripts.zip 唯一读取顺序
├── wiki/                 # MediaWiki API client
│   ├── mod.rs
│   └── client.rs         # WikiClient (login, get_page, edit_page, append/prepend)
├── copyclip/             # marker-based wiki module content updater
│   ├── mod.rs            # CopyClipProcessor
│   └── config.rs         # TOML config for module constants
├── scripts_sync/         # scripts.zip 同步 (update_scripts.py 移植)
│   ├── mod.rs            # sync(): 版本检测→staging 解压→快照归档→版本记录
│   ├── anim/             # anim-sync/anim-diff + anim-index + 重映射管线
│   │   ├── remap_history.rs # 重映射快照(history/remaps/<label>.json)+结构化 diff
│   │   ├── index/        # anim-index: prefab↔动画索引 + anim-remap-index.json(Tier A/B/C)
│   │   │   ├── mod.rs    # 类型 + run_index + 测试
│   │   │   └── scanner.rs # Lua 扫描器
│   │   ├── preview.rs    # 渲染端点后端(SymbolOverrideMap/skin/override build 自动加载)
│   │   └── ...
│   ├── images/           # 图片管线 images-sync: 两源盘点→解压→内置解码→切割→差异历史
│   │   ├── mod.rs        # run() 编排 + ImagesSyncParams + 对账清理
│   │   ├── scan.rs       # zip+loose 两源扫描 → 合并视图 (xml↔tex 联接)
│   │   ├── ktex.rs       # KTEX 容器解析 + DXT1/3/5/RGB 解码(texpresso) + 反预乘
│   │   ├── split.rs      # ktools atlas XML 解析 + UV 裁剪 (v 轴翻转)
│   │   ├── meta.rs       # 物品图标元数据: icon_meta.json + 文件名映射表 + 五态标题/状态
│   │   └── history.rs    # CAS 对象仓 + manifest + diff 纯函数
│   └── state.rs          # dst_version.txt 状态文件 + 版本对比纯函数
├── context.rs            # DstContext (lazy ZIP archive + wiki client + env)
├── error.rs              # Error enum (14 variants) + Result<T> alias
└── utils.rs              # diff_lines() only (unified-diff utility)
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Add re-export of a new type | `src/lib.rs` | Both `pub mod` + `pub use` needed |
| Understand module dependency order | `src/lib.rs` lines 1-16 | Declaration order is the compile order |
| Trace what lib.rs re-exports | `src/lib.rs` lines 18-26 | DstContext, SnapshotInfo, CopyClip types, Error/Result, TechReport, diff helpers |
| Add a binary-only entry point | `src/main.rs` | `mod commands` + match on args.command |
| Add a new CLI command variant | `src/service/job_spec.rs` (`JOB_SPECS` 行) + `src/commands/mod.rs` (`CLI_EXT` 行 + `job_from_matches` 构造臂) | 契约测试强制 name/serde tag/CLI 旗标/前端 JOB_DEFS 一致 |
| Check all Error variants | `src/error.rs` lines 4-46 | Io, PoParse, InvalidPoEntry, EnvVarNotFound, ParseError, Http, WikiApi, LoginFailed, EditFailed, Config, Zip, ArchiveFileNotFound, DstDirNotFound, Json |
| Understand file vs directory modules | `src/` root | context.rs/error.rs/utils.rs = single files; everything else = directory |
| Find where a concrete mapper lives | `src/mapping/mappers/` | Each mapper is its own file |
| Update CopyClip wiki module config | `src/copyclip/config.rs` | TOML-based module/page mappings |
| Change env/path defaults | `src/platform/config.rs` | 所有路径类环境变量集中解析（含 `ICON__TITLE_OVERRIDES`） |
| Atomic/JSON artifact writes | `src/platform/fs.rs` | `write_text_atomic`/`write_json_atomic`；避免 Web 读到半截 JSON |
| Read game data (snapshot/live/zip) | `src/platform/game_source.rs` | 唯一读取顺序定义；新增读取一律走 `GameSource` |
| Sync scripts after a game update | `src/scripts_sync/` | `scripts-sync`; snapshot naming must stay `scripts_<yyyymmddhhmm>` for `DstContext::list_snapshots` |
| 维护 模块:Strings 桶页 | `src/service/strings_wiki.rs` | `maintain-strings`：PO key 大写归一 + CHARACTERS 角色表合并 → 分桶 → 语义对比后只写变化页、索引最后写（`--dry-run` 只读）；纯逻辑在 `src/models/strings.rs`，桶页解析在 `src/parser/strings_data.rs` |

## LOCAL CONVENTIONS
- **Module files**: `mod.rs` only in directory modules. No other files contain `mod` declarations.
- **lib.rs discipline**: Declarations + re-exports only. No inline code, no mod tests.
- **commands exclusion**: `commands` stays out of lib.rs to keep clap deps out of the library crate.
- **Re-export policy**: Only the types external consumers need. Internal types stay `pub(crate)` or private.

## ANTI-PATTERNS (src/ specific)
- **DO NOT** add `commands` to lib.rs -- clap types would leak into the library crate
- **DO NOT** convert `context.rs`, `error.rs`, or `utils.rs` into directories -- they're intentionally flat
- **DO NOT** import `crate::commands` from any lib module -- it won't compile (commands is binary-only)
- **DO NOT** add new standalone `.rs` files at `src/` root -- use a subdirectory module instead
- **DO NOT** re-export everything from lib.rs -- only the key interface types
- **DO NOT** put `mod tests` in lib.rs -- tests go in the individual module files
