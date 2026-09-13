# PROJECT KNOWLEDGE BASE

**Generated:** 2026-06-30
**Commit:** 08ced97
**Branch:** master

## OVERVIEW
Rust CLI tool for maintaining the Don't Starve Together (DST) Huiji Wiki. Parses game data (Lua, PO) → maps to wiki JSON schema → pushes via MediaWiki API.

## STRUCTURE
```
dst-huiji-wiki/
├── src/
│   ├── main.rs              # Binary entry (tokio + clap dispatch)
│   ├── lib.rs               # Library root (10 public modules)
│   ├── commands/            # CLI arg definitions + all handlers
│   ├── web/                 # WebUI server (binary-only; axum routes, JobManager, embedded SPA assets)
│   ├── parser/              # Game data parsers (Lua, PO, recipes, prefab overrides)
│   │   └── prefab_override/ # Complex Lua AST walker (parser/ 按职责分模块: mod/analysis/deep)
│   ├── models/              # Data models (Recipe, PoEntry, TechReport)
│   │   └── recipe/          # Recipe sub-models (ingredient, options, prototyper, context)
│   ├── mapping/             # Data→wiki mapping framework (WikiMapper trait + 字段名对齐 diff/merge)
│   │   └── mappers/         # Concrete mappers (PoEntryMapper, RecipeMapper)
│   ├── wiki/                # MediaWiki API client
│   ├── copyclip/            # Wiki module content updater (marker-based replacement)
│   ├── scripts_sync/        # scripts.zip 同步:版本检测→归档旧树→解压新树 (update_scripts.py 移植)
│   ├── context.rs           # DstContext (组合 GameSource + wiki client + env setup)
│   ├── error.rs             # Error enum (thiserror) + Result<T>
│   └── utils.rs             # diff_lines utility
├── examples/                # Test data files (.po, .lua, .json) + login.py
├── docs/                    # Design docs (PLAN.md, REFACTORING.md)
└── .github/workflows/      # CI (test+lint+build) + Release (4 cross-compile targets)
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Add a new CLI command | `src/service/job_spec.rs` (JobSpec 行) + `src/commands/mod.rs` (CLI_EXT 行 + `job_from_matches` 构造臂) | 子命令由 clap builder 从 JobSpec+CLI_EXT 生成；参数 schema 声明在 spec |
| Add a new job type (CLI+Web) | `src/service/kind.rs` (variant) + `src/service/job_spec.rs` (spec 行) + `src/commands/mod.rs` (CLI_EXT 行 + 构造臂) | 契约测试强制四处一致（name/serde tag/CLI 旗标/前端 JOB_DEFS）|
| Change WebUI behavior | `src/web/` | Binary-only module like commands; assets in `src/web/assets/` embedded via include_str! |
| Add a data browse endpoint | `src/web/api_data/{dataset,icons,assets,anim}.rs` + `src/service/dataset.rs` | Dataset is cached per snapshot in memory |
| Parse a new Lua data type | `src/parser/` | Add new module, re-export in mod.rs |
| Add a new wiki data mapping | `src/mapping/mappers/` | Implement WikiMapper trait |
| Change wiki API interaction | `src/wiki/client.rs` | All HTTP/API logic here (throttle+retry in `send_with_retry`) |
| Change write/confirm policy | `src/platform/progress.rs` (`WriteMode`, `decide_write`) + `src/service/wiki_write.rs` (`WikiWriter`) | CLI --yes/--dry-run → WriteMode；写站流程统一走 WikiWriter（确认/decide_write/basetimestamp/span）|
| Add a data model | `src/models/` | Add struct + serde derives |
| Update CopyClip (module constants) | `src/copyclip/` | TOML config in config.rs; `--type names` also derives CraftingNames station aliases from `constants.lua`/`tuning.lua` (`parse_*` in `src/parser/crafting.rs`, logic in `src/models/crafting_alias.rs`) |
| Check 模板:Tech/dst & 模板:制作栏图标 coverage | `src/service/template_check.rs` | `maintain-template-check` CLI/Web job (read-only); config `config/template_check.json` (tech whitelist / CN alias / station fallback); snippets via `--output` |
| Parse/modify wikitext | `src/wikitext/` + `src/service/wikitext_edit.rs` | Lossless round-trip DOM per docs/WIKITEXT_PARSER_PLAN.md: `Wikicode::parse`/`serialize`（永不失败）、`templates_named`/`param`/`sections`/`switch_cases` 读、`set_param`/`add_param`/`remove_param` 格式保持写（节点持字符串 + 字节 span，编辑后 span 作废）；可变遍历是回调式（`with_templates_named_mut`）；`maintain-wikitext` CLI 批量改模板参数；`segment.rs` 信息框提取已迁移并保留旧实现作差分 |
| 维护 模块:Strings 桶页 | `src/service/strings_wiki.rs` | `maintain-strings`：解析 pot/po → key 大写归一 + 角色表合并 → 沿用现有索引边界分桶 → 逐桶语义对比后只写变化页，索引最后写（`--dry-run` 只产报告，`--limit N` canary 不写索引）；`src/models/strings.rs`（变换/分桶/渲染）、`src/parser/strings_data.rs`（桶页解析） |
| Harvest the wiki corpus | `src/corpus/` + `service::JobKind::CorpusFetch` | `corpus-fetch` CLI; layout/classifier per docs/WIKI_CORPUS_PLAN.md; output in gitignored `wikis/`; `corpus-index` rebuilds derived indexes (prefab registry / regions / facts) per docs/CORPUS_CODE_ATLAS_CONTRACT.md |
| Sync scripts after a game update | `src/scripts_sync/` | `scripts-sync` CLI; archives live tree as `scripts_<ts>` snapshot (consumed by `DstContext::list_snapshots`), extracts `scripts.zip`, records version in `dst_version.txt`; image pipeline ported as `images-sync` (see below) |
| Fix prefab name extraction | `src/parser/prefab_override/parser/` | `mod.rs`(入口/收集) + `analysis.rs`(工厂/表分析) + `deep.rs`(跨函数深度解析)；行为由 examples 指纹测试钉住 |
| Edit the wiki skilltree renderer | `src/service/assets/skilltree_widget.js` | 零件:Skilltree.js source; `skilltree-wiki --output` emits it as `Skilltree.js`; must stay in sync with `src/web/assets/app.js` skilltree section |
| Upload an image | `src/service/upload_image.rs` | `upload-image` CLI; auto description by dir (`skilltree/`→技能树素材, `skilltree_icons/`→技能树图标, `inventoryimages/`→物品栏图标); `--ignore-warnings` for re-upload (needs `reupload` right) |
| Upload inventory icons / edit wiki file names | `src/service/upload_icons.rs` + `src/scripts_sync/images/meta.rs` | `upload-icons` CLI (`--file`+`--title` manual, `--source inventory\|crafting`); WebUI 物品栏/制作栏图标两个独立页面（制作栏按 `icon_meta.json` 的 `crafting.kind` 分节；物品栏按 build 历史分组）+ 弹窗编辑映射; icon sources in `src/scripts_sync/images/icons.rs` (`inventoryimages` / `crafting_menu_icons`); override table `config/icon_title_overrides.json` (local file name → wiki file name), applied by `images-sync` into `history/icon_meta.json` |
| Add environment config | `.env.example` → `.env` | HUIJI__*, DST__ROOT, KTOOLS__OUT_DIR (images-sync), ICON__TITLE_OVERRIDES vars |

## CODE MAP

| Symbol | Type | Location | Role |
|--------|------|----------|------|
| `DstContext` | Struct | src/context.rs | App context: `GameSource`（snapshot→live→zip）+ wiki client + env |
| `JobKind` / `JobSpec` | Enum / 静态表 | src/service/kind.rs + src/service/job_spec.rs | 作业参数变体 + canonical name/wiki_access/resources/params 声明源 |
| `WikiWriter` | Struct | src/service/wiki_write.rs | 写站流程唯一实现（diff/确认/decide_write/basetimestamp） |
| `platform::*` | Module | src/platform/ | progress（Reporter/WriteMode）、config（env 路径集中）、fs（原子写）、game_source（snapshot→live→zip） |
| `WikiMapper` | Trait | src/mapping/mapper.rs | Core mapping interface: schema + rules + merge |
| `WikiDataConverter` | Struct | src/mapping/converter.rs | Orchestrates mapping: parse → convert → compare → merge（diff/merge 按字段名对齐，显式 `key_field`）|
| `RecipeParser` | Struct | src/parser/recipe.rs | Parses Recipe{} calls from Lua via full_moon AST |
| `PrefabOverrideParser` | Struct | src/parser/prefab_override/parser/mod.rs | Extracts prefab name overrides from Lua (factory patterns, control flow) |
| `LuaParser` | Struct | src/parser/lua.rs | Generic Lua variable/field location extraction |
| `PoParser` | Struct | src/parser/po.rs | Nom-based PO file parser |
| `WikiClient` | Struct | src/wiki/client.rs | MediaWiki API: login, get_page, edit_page, append/prepend, get_files_info (imageinfo), upload_file (multipart) |
| `CopyClipProcessor` | Struct | src/copyclip/mod.rs | Marker-based content replacement in wiki modules |
| `Recipe` | Struct | src/models/recipe/mod.rs | Game recipe: name, ingredients, tech, options |
| `PoEntry` / `PoFile` | Struct | src/models/po.rs | PO translation entry + file container |
| `TechReport` | Struct | src/models/tech_report.rs | Compares parsed vs wiki tech levels |
| `Error` | Enum | src/error.rs | 14 variants (Io, PoParse, Http, WikiApi, Zip, etc.) |
| `SyncParams` / `sync()` | Struct / Fn | src/scripts_sync/mod.rs | scripts.zip 同步入口:版本检测→staging 解压→快照归档→版本记录 |
| `images::run()` / `ImagesSyncParams` | Fn / Struct | src/scripts_sync/images/mod.rs | 图片管线 images-sync:两源盘点→解压 images.zip→内置 KTEX 解码(ktex-rs)→xml 切割;最终产物入 CAS 差异历史 |
| `ObjectStore` / `Manifest` / `diff_final_maps` | Struct / Fn | src/scripts_sync/images/history.rs | 内容寻址对象仓 + 每 build 全量清单 + 相邻 diff 纯函数 |

## CONVENTIONS
- **Edition 2021**, stable Rust only, no nightly features
- **No `rustfmt.toml`/`clippy.toml`** — all defaults. CI enforces `cargo fmt --check` + `cargo clippy -D warnings`
- **Inline tests only**: `#[cfg(test)] mod tests { use super::*; }` — no `tests/` directory, no dev-dependencies
- **No `unsafe`** anywhere in the codebase
- **Error handling**: `thiserror` derives → `crate::error::Result<T>` alias everywhere
- **Dependencies**: major version only (no pinning), semver-compatible ranges
- **`serde_json` with `preserve_order`** — JSON output maintains insertion order
- **`full_moon` with `lua52`** — Lua 5.2 dialect for DST game scripts
- **CLI args**: clap **builder** generated from `JobSpec` + `CLI_EXT` tables (no derive enum); `r#type` raw identifier for `--type` flag in copyclip

## ANTI-PATTERNS (THIS PROJECT)
- **DO NOT** add `rustfmt.toml` or `clippy.toml` — project uses Rust defaults
- **DO NOT** create `tests/` directory — inline `#[cfg(test)]` is the decided convention (see docs/REFACTORING.md)
- **DO NOT** use `unwrap()` outside tests — use `Result` propagation with `?`
- **DO NOT** add `unsafe` blocks — none exist, none should be needed
- **DO NOT** put Rust examples in `examples/` — it contains game data fixtures, not runnable examples

## UNIQUE STYLES
- `WikiMapper` trait: declarative mapping via `FieldMapping` enum (Direct/Transformed/Computed/Constant/Default/Ignored) + `MergeStrategy` (Overwrite/PreserveHistory/Merge/Custom)
- `mapping::converter`: diff/merge 按【字段名】在两侧 schema 各自定位并对齐（不依赖字段下标/位置）；`compare_data`/`merge_new_records` 必须显式传入 `T::key_field()`
- `CopyClipProcessor`: marker-based content replacement (`--BEGIN/--END`) in wiki Lua modules
- `DstContext`: composes `platform::game_source::GameSource` (唯一 snapshot→live→zip 读取语义) with the wiki client; provides PO/Lua access
- `RecipeParser`: handles Lua for-loops (numeric + generic/ipairs) to expand recipe definitions at parse time
- `PrefabOverrideParser`: deep AST walking across function boundaries, tracks local variables and parameters to resolve prefab name overrides
- `scripts_sync`: staging-first sync (extract to `incoming_<ts>` → rename live tree to snapshot → move staged tree in place, rollback on failure); timestamp collision degrades to a clear error + hint, no auto-suffix

## COMMANDS
```bash
cargo build --release              # Build binary
cargo test                         # Run all ~640 lib + ~20 bin inline tests
cargo fmt --check                  # Check formatting
cargo clippy -- -D warnings        # Lint (CI uses this)
cargo run --release -- --help      # Show CLI help
```

## NOTES
- `.env` required for wiki operations (HUIJI__USERNAME, HUIJI__PASSWORD, HUIJI__X_AUTHKEY, DST__ROOT)
- WikiClient: global throttle (default 1 QPS, `WIKI__QPS`) + retry on 403/429/GET-5xx (`WIKI__MAX_RETRIES`); POST only retries WAF-level 403/429
- Edits carry `basetimestamp` + `assert=user`; conflicts surface as `Error::EditConflict`
- Batch APIs: `get_pages_meta` (≤50 titles), `page_exists`, `list_all_pages` (continuation-aware)
- File lookup: `get_files_info`/`file_exists`/`get_file_url` via `prop=imageinfo&iiprop=url` (≤50 File: titles); `file_title` mirrors MediaWiki title normalization (first letter upper-cased, `_`≡space, `File:`/`文件:` prefix)
- Error variants RateLimited/PageNotFound/AuthExpired/EditConflict + `Error::is_retryable()` for automated branching
- Maintenance commands accept `--yes` (auto-confirm writes), `--dry-run` (no wiki writes, artifacts still written), `--report-json <path>` (machine-readable report); service-level enum is `service::WriteMode` with pure decision fn `decide_write`
- Structured logging: `cli_run` span (uuid run_id, command) wraps every invocation; `job` and `wiki_edit` (page, oldrevid/newrevid) spans inside service
- `examples/` contains game data (.po, .lua) + a Python login script, NOT Rust examples — `cargo run --example` finds nothing
- `src/commands/mod.rs` is builder-generated CLI from JobSpec + CLI_EXT tables (no derive enum)
- `src/parser/prefab_override/parser/` 已按职责拆为 mod/analysis/deep（原 3145 行单文件）；`src/parser/skilltree/` 拆为 mod/scan/eval/cond/analyze
- CI cross-compiles to 4 targets (Linux x86_64, macOS x86_64 + aarch64, Windows x86_64)
- Wiki API tests skip gracefully if `.env` not configured (won't fail in CI)
- `reqwest = "0.13"` — newer than typical 0.12.x; verify intentional
- No `rust-toolchain.toml` — CI uses `dtolnay/rust-toolchain@stable`
