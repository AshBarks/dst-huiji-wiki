# src/ AGENTS.md

**Scope**: All library code. Binary entrypoint is a thin shell.

## OVERVIEW
10 public modules (lib.rs) + 1 binary-only module (main.rs:commands). Standalone files at root: `context.rs`, `error.rs`, `utils.rs`. Directory modules: `commands/`, `parser/`, `models/`, `mapping/`, `wiki/`, `copyclip/`, `corpus/`, `knowledge/`, `llm/`, `update/`, `scripts_sync/`, `service/`.

## STRUCTURE
```
src/
├── main.rs               # bin: mod commands + dispatch to commands::run()
├── lib.rs                # 10 pub mod declarations + pub use of key types
├── commands/             # binary-only (NOT in lib.rs)
│   ├── mod.rs            # Args (clap derive) + Commands enum + run() dispatcher
│   └── maintain.rs       # 7+ command handlers (743 lines)
├── parser/               # game data parsers
│   ├── mod.rs
│   ├── lua.rs            # LuaParser (variable/field location)
│   ├── po.rs             # PoParser (nom-based PO)
│   ├── recipe.rs         # RecipeParser (full_moon AST, matches Recipe2 calls)
│   ├── skilltree.rs      # skill tree extractor (pos/connects with constant folding)
│   └── prefab_override/  # PrefabOverrideParser (2657 lines)
├── models/               # data model structs with serde derives
│   ├── mod.rs
│   ├── po.rs             # PoEntry, PoFile
│   ├── recipe/           # Recipe + sub-models (ingredient, options, prototyper, context)
│   └── tech_report.rs    # TechReport
├── mapping/              # data to wiki mapping framework
│   ├── mod.rs
│   ├── mapper.rs         # WikiMapper trait (FieldMapping + MergeStrategy)
│   ├── builder.rs        # MappingBuilder<T> fluent API
│   ├── converter.rs      # WikiDataConverter orchestration
│   ├── schema.rs         # wiki JSON schema types
│   └── mappers/          # concrete impls (PoEntryMapper, RecipeMapper)
├── wiki/                 # MediaWiki API client
│   ├── mod.rs
│   └── client.rs         # WikiClient (login, get_page, edit_page, append/prepend)
├── copyclip/             # marker-based wiki module content updater
│   ├── mod.rs            # CopyClipProcessor
│   └── config.rs         # TOML config for module constants
├── scripts_sync/         # scripts.zip 同步 (update_scripts.py 移植)
│   ├── mod.rs            # sync(): 版本检测→staging 解压→快照归档→版本记录
│   ├── images/           # 图片管线 images-sync: 两源盘点→解压→内置解码→切割→差异历史
│   │   ├── mod.rs        # run() 编排 + ImagesSyncParams + 对账清理
│   │   ├── scan.rs       # zip+loose 两源扫描 → 合并视图 (xml↔tex 联接)
│   │   ├── ktex.rs       # KTEX 容器解析 + DXT1/3/5/RGB 解码(texpresso) + 反预乘
│   │   ├── split.rs      # ktools atlas XML 解析 + UV 裁剪 (v 轴翻转)
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
| Understand module dependency order | `src/lib.rs` lines 1-8 | Declaration order is the compile order |
| Trace what lib.rs re-exports | `src/lib.rs` lines 10-17 | DstContext, CopyClip types, Error/Result, TechReport, diff_lines |
| Add a binary-only entry point | `src/main.rs` | `mod commands` + match on args.command |
| Add a new CLI command variant | `src/commands/mod.rs` | Add to Commands enum -> handler in maintain.rs |
| Check all Error variants | `src/error.rs` lines 4-46 | Io, PoParse, InvalidPoEntry, EnvVarNotFound, ParseError, Http, WikiApi, LoginFailed, EditFailed, Config, Zip, ArchiveFileNotFound, DstDirNotFound, Json |
| Understand file vs directory modules | `src/` root | context.rs/error.rs/utils.rs = single files; everything else = directory |
| Find where a concrete mapper lives | `src/mapping/mappers/` | Each mapper is its own file |
| Update CopyClip wiki module config | `src/copyclip/config.rs` | TOML-based module/page mappings |
| Sync scripts after a game update | `src/scripts_sync/` | `scripts-sync`; snapshot naming must stay `scripts_<yyyymmddhhmm>` for `DstContext::list_snapshots` |

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
