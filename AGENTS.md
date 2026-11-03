# PROJECT KNOWLEDGE BASE

**Generated:** 2026-11-02
**Commit:** e5b90d8
**Branch:** master

## OVERVIEW
Rust CLI tool for maintaining the Don't Starve Together (DST) Huiji Wiki. Parses game data (Lua, PO) → maps to wiki JSON schema → pushes via MediaWiki API.

## STRUCTURE
```
dst-huiji-wiki/
├── src/
│   ├── main.rs              # Binary entry (tokio + clap dispatch; `mod commands` + `mod web`)
│   ├── lib.rs               # Library root (15 public modules；wikitext/ktex 已移至 crates/)
│   ├── commands/            # CLI 构建与分发（binary-only）：JobSpec + CLI_EXT → clap builder
│   ├── web/                 # WebUI server（binary-only）：axum 路由、JobManager、内嵌 SPA
│   ├── service/             # 作业编排层：JobKind 分发 + 各维护作业实现
│   ├── platform/            # 业务无关基础设施：Reporter/WriteMode、config、fs、GameSource
│   ├── parser/              # 游戏数据解析（Lua/PO/配方/预制体/技能树/动画）
│   │   └── prefab_override/ # 复杂 Lua AST walker（按职责分 mod/analysis/deep）
│   ├── models/              # 数据模型（Recipe、PoEntry、TechReport、strings…）
│   │   └── recipe/          # Recipe sub-models (ingredient, options, prototyper, context)
│   ├── mapping/             # 数据→wiki 映射框架（WikiMapper + 字段名对齐 diff/merge）
│   │   └── mappers/         # Concrete mappers (PoEntryMapper, RecipeMapper)
│   ├── wiki/                # MediaWiki API 客户端（client/{mod,transport,endpoints}）
│   ├── copyclip/            # 模块常量区间替换（marker-based）
│   ├── scripts_sync/        # scripts.zip / 图片 / 动画同步管线（images/meta 已拆 title/names/build）
│   ├── corpus/              # wiki 语料采集与索引（corpus-fetch / corpus-index）
│   ├── update/              # 更新影响评估：关联图集、快照 diff、Tier0 规则、符号标注
│   ├── knowledge/           # 知识库流水线：符号/页面扫描、分类、同步
│   ├── llm.rs               # OpenAI 兼容 LLM 客户端（SSE 流式）
│   ├── context.rs           # DstContext (GameSource + wiki client + env)
│   ├── error.rs             # Error enum (thiserror) + Result<T>
│   └── utils.rs             # diff_lines 等工具
├── crates/                  # Cargo workspace 成员
│   ├── dst-wikitext/        # 独立 crate：wikitext 无损解析/编辑（零依赖，可发布）
│   ├── dst-ktex/            # 独立 crate：KTEX 解析/解码（app 与 dst-anim-tool 共用）
│   └── dst-anim-tool/       # 收编的 DST 动画工具（app 仅用 default-features=false + gif）
├── examples/                # 测试用游戏数据 (.po, .lua, .json) + login.py（examples/prefabs 不入库）
├── docs/                    # 设计文档；先看 docs/INDEX.md（active/参考/历史标注）
└── .github/workflows/      # CI (test+lint+build) + Release (4 cross-compile targets)
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Add a new CLI command | `src/service/job_spec.rs` (JobSpec 行) + `src/commands/mod.rs` (`CLI_EXT` 行，只写与缺省不同的字段) | 子命令由 clap builder 从 JobSpec+CLI_EXT 生成；`JobKind` 由 spec 表驱动构造（无手写臂），`all_variants()` 同样由表生成 |
| Run post-update orchestration | `src/service/pipelines.rs` | `update-local` (scripts→images→anim) / `update-data` / `update-assets` / `update-check`; `--from <step>` resume; order constraints in `docs/UPDATE_TASK_GROUPS.md` |
| Add a new job type (CLI+Web) | `src/service/kind.rs` (variant) + `src/service/job_spec.rs` (spec 行 + `web_ui`) + `src/commands/mod.rs` (CLI_EXT 行) + `src/service/mod.rs` (dispatch 臂) | 契约测试强制一致（name/serde tag/CLI 旗标/spec↔变体字段类型）；`web_ui=true` 即由 `/api/jobs/specs` 自动出现在 WebUI，特殊控件才写 `JOB_UI` 布局 |
| Change WebUI behavior | `src/web/` | Binary-only module like commands; assets in `src/web/assets/` embedded via include_str! |
| Add a data browse endpoint | `src/web/api_data/{dataset,icons,assets,anim}.rs` + `src/service/dataset.rs` | Dataset is cached per snapshot in memory |
| Preview followed FX (FollowSymbol anchors) | `docs/ANIM_FOLLOW_FX.md` + `src/parser/follow_symbol.rs` + `src/scripts_sync/anim/preview.rs` + `crates/dst-anim-tool/src/render.rs` | 挂点 symbol 无 build 贴图，用"挂点矩阵 × 跟随动画"合成；Lua 索引优先（含跨文件 SpawnPrefab 回填）、同 bank 启发式兜底 |
| Parse a new Lua data type | `src/parser/` | Add new module, re-export in mod.rs |
| Add a new wiki data mapping | `src/mapping/mappers/` | Implement WikiMapper trait |
| Change wiki API interaction | `src/wiki/client/{transport,endpoints}.rs` | transport：登录/GET/POST/CSRF + 节流重试（`send_with_retry`）；endpoints：页面/文件/编辑/上传与响应解析 |
| Change write/confirm policy | `src/platform/progress.rs` (`WriteMode`, `decide_write`) + `src/service/wiki_write.rs` (`WikiWriter`) | CLI --yes/--dry-run → WriteMode；写站流程统一走 WikiWriter（确认/decide_write/basetimestamp/span）|
| Add a data model | `src/models/` | Add struct + serde derives |
| Update CopyClip (module constants) | `src/copyclip/` | TOML config in config.rs; `--type names` also derives CraftingNames station aliases from `constants.lua`/`tuning.lua` (`parse_*` in `src/parser/crafting.rs`, logic in `src/models/crafting_alias.rs`) |
| Check 模板:Tech/dst & 模板:制作栏图标 coverage | `src/service/template_check.rs` | `maintain-template-check` CLI/Web job (read-only); config `config/template_check.json` (tech whitelist / CN alias / station fallback); snippets via `--output` |
| Parse/modify wikitext | `crates/dst-wikitext/` + `src/service/wikitext_edit.rs` | Lossless round-trip DOM per docs/WIKITEXT_PARSER_PLAN.md: `Wikicode::parse`/`serialize`（永不失败）、`templates_named`/`param`/`sections`/`switch_cases` 读、`set_param`/`add_param`/`remove_param` 格式保持写（节点持字符串 + 字节 span，编辑后 span 作废）；可变遍历是回调式（`with_templates_named_mut`）；`maintain-wikitext` CLI 批量改模板参数；`segment.rs` 信息框提取已迁移并保留旧实现作差分 |
| 维护 模块:Strings 桶页 | `src/service/strings_wiki.rs` | `maintain-strings`：解析 pot/po → key 大写归一 + 角色表合并 → 沿用现有索引边界分桶 → 逐桶语义对比后只写变化页，索引最后写（`--dry-run` 只产报告，`--limit N` canary 不写索引）；`src/models/strings.rs`（变换/分桶/渲染）、`src/parser/strings_data.rs`（桶页解析） |
| Harvest the wiki corpus | `src/corpus/` + `service::JobKind::CorpusFetch` | `corpus-fetch` CLI; layout/classifier per docs/WIKI_CORPUS_PLAN.md; output in gitignored `wikis/`; `corpus-index` rebuilds derived indexes (prefab registry / regions / facts) per docs/CORPUS_CODE_ATLAS_CONTRACT.md |
| Sync scripts after a game update | `src/scripts_sync/` | `scripts-sync` CLI; archives live tree as `scripts_<ts>` snapshot (consumed by `DstContext::list_snapshots`), extracts `scripts.zip`, records version in `dst_version.txt`; image pipeline ported as `images-sync` (see below) |
| Export the cooking minigame bundle | `src/service/cooking_export.rs` | `cooking-game-export` CLI; payload/AST parser in `src/parser/cooking/` + `src/service/cooking*.rs`, bundle layout & schema versions in docs/COOKING_SIMULATOR.md; recipe `foodtype` from `preparedfoods*.lua`, entry-instant `stats`/`temperature` from `preparedfoods*.lua` + `tuning.lua` (whitelist evaluator in `src/parser/cooking/tuning.rs`), character roster+portraits from `saveslot_portraits/` |
| Fix prefab name extraction | `src/parser/prefab_override/parser/` | `mod.rs`(入口/收集) + `analysis.rs`(工厂/表分析) + `deep.rs`(跨函数深度解析)；行为由 examples 指纹测试钉住 |
| Edit the wiki skilltree renderer | `src/service/assets/skilltree_widget.js` | 零件:Skilltree.js 的**唯一**实现；`skilltree-wiki --output` 输出为 `Skilltree.js`，WebUI 预览经 `/static/js/skilltree_widget.js` 复用同一渲染器（`skills.js` 只做数据/素材适配） |
| Upload an image | `src/service/upload_image.rs` | `upload-image` CLI; auto description by dir (`skilltree/`→技能树素材, `skilltree_icons/`→技能树图标, `inventoryimages/`→物品栏图标); `--ignore-warnings` for re-upload (needs `reupload` right) |
| Upload icons / edit wiki file names | `src/service/upload_icons.rs` + `src/scripts_sync/images/meta/` | `upload-icons` CLI (`--file`+`--title` manual, `--source inventory\|crafting\|skilltree`); WebUI 物品栏/制作栏/技能树图标三个仪表盘页（制作栏按 `icon_meta.json` 的 `group.section`，技能树按 `group.character`；物品栏按 build 历史）+ 弹窗编辑映射; icon sources in `src/scripts_sync/images/icons.rs` (`inventoryimages` / `crafting_menu_icons` / `skilltree_icons`); override table `config/icon_title_overrides.json` (local file name → wiki file name，`SKILLTREE__ICON_NAMES` 可覆盖技能树例外表), applied by `images-sync` into `history/icon_meta.json`; 技能树分类对账写 `wiki.in_category`，重定向占位页仅报告、不覆盖 |
| Add environment config | `.env.example` → `.env` | HUIJI__*, DST__ROOT, KTOOLS__OUT_DIR (images-sync), KNOWLEDGE__ROOT (知识库本地数据), ICON__TITLE_OVERRIDES / SKILLTREE__ICON_NAMES |

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
| `WikiClient` | Struct | src/wiki/client/mod.rs | MediaWiki API: login, get_page, edit_page, append/prepend, get_files_info (imageinfo), upload_file (multipart)；传输层在 `client/transport.rs`，端点在 `client/endpoints.rs` |
| `CopyClipProcessor` | Struct | src/copyclip/mod.rs | Marker-based content replacement in wiki modules |
| `Recipe` | Struct | src/models/recipe/mod.rs | Game recipe: name, ingredients, tech, options |
| `PoEntry` / `PoFile` | Struct | src/models/po.rs | PO translation entry + file container |
| `TechReport` | Struct | src/models/tech_report.rs | Compares parsed vs wiki tech levels |
| `Error` | Enum | src/error.rs | 14 variants (Io, PoParse, Http, WikiApi, Zip, etc.) |
| `SyncParams` / `sync()` | Struct / Fn | src/scripts_sync/mod.rs | scripts.zip 同步入口:版本检测→staging 解压→快照归档→版本记录 |
| `images::run()` / `ImagesSyncParams` | Fn / Struct | src/scripts_sync/images/mod.rs | 图片管线 images-sync:两源盘点→解压 images.zip→KTEX 解码(dst-ktex)→xml 切割;最终产物入 CAS 差异历史 |
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
- **DO NOT** create `tests/` directory — inline `#[cfg(test)]` is the decided convention（见 docs/STRUCTURE_OPT_PLAN.md）
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
cargo build --release              # 构建 app 二进制（default-members=.）
cargo test --workspace --exclude dst-anim-tool   # app + dst-wikitext 全量测试（CI 同款）
cargo test -p dst-huiji-wiki --lib -- --include-ignored  # 本地全量 app 测试（需 examples/prefabs；conformance 自动跳过）
cargo test -p dst-wikitext         # 只测抽出的 wikitext crate
cargo test -p dst-ktex             # 只测抽出的 KTEX crate（合成 fixture，无需游戏素材）
cargo test -p dst-anim-tool --no-default-features --features cli,gif  # anim-tool（CI 模式：50 个数据用例 ignored）
cargo test -p dst-anim-tool --no-default-features --features cli,gif -- --include-ignored  # 本地全量（需 data/anim 软链）
cargo fmt --all --check            # Check formatting
cargo clippy --workspace --exclude dst-anim-tool --all-targets --all-features -- -D warnings  # Lint (CI uses this)
cargo deny check                   # Dependencies: licenses/bans/sources/advisories (deny.toml; needs cargo-deny)
cargo run --release -- --help      # Show CLI help
```

## NOTES
- Workspace：根 package 是 app，`crates/*` 为成员；`default-members = ["."]`（裸 `cargo build`/`cargo test` 只作用于 app）
- `crates/dst-wikitext`：零第三方依赖的 wikitext 无损解析/编辑 crate（可发布），app 通过 workspace dep 引用
- `crates/dst-ktex`：KTEX 解析/解码共享 crate（app 与 dst-anim-tool 共用；`build_tex`/`compress_bc3` 可造合成 fixture）；`DECODER_VERSION` 变更会触发图片历史全量重处理
- `crates/dst-anim-tool`：git subtree 收编（保留独立历史），app 以 `default-features = false, features = ["gif"]` 引用
- app 的 6 个 prefab_override 用例依赖本机 `examples/prefabs/`（已 gitignore，CI 不可用），标 `#[ignore = "requires local DST game data (examples/prefabs)"]`；本地 `cargo test -p dst-huiji-wiki --lib -- --include-ignored` 跑全量（含指纹测试 `test_all_prefabs_files`）
- anim-tool 的 50 个用例依赖本机 DST 游戏资源，已标 `#[ignore = "requires local DST game data (data/anim symlink)"]`；
  CI 跑其余 72 个。本地建软链后跑全量：
  `mkdir -p crates/dst-anim-tool/data && ln -s "<DST>/data/anim" crates/dst-anim-tool/data/anim`
  然后 `cargo test -p dst-anim-tool --no-default-features --features cli,gif -- --include-ignored`
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
- 依赖治理：根 `deny.toml`（许可证允许清单 + advisories/bans/sources）；`zip` 关闭默认特性只留 `deflate`，并把 app 与 anim-tool 都升到 zip 9（8.x 的 extract 公告）
- No `rust-toolchain.toml` — CI uses `dtolnay/rust-toolchain@stable`
- `knowledge/` 是**本地数据**（已 gitignore，不入库）；`KNOWLEDGE__ROOT` 可整体重定向，缺省 `knowledge/`
- 技能树渲染器单一源：`src/service/assets/skilltree_widget.js` 同时服务维基与 WebUI 预览（`/static/js/skilltree_widget.js` + `window.skilltreeAssetUrl` 素材注入）
- 作业登记：`JobKind` 变体（`kind.rs`）+ spec 表（`job_spec.rs`）+ CLI 视角（`commands` 的 `CLI_EXT`）+ dispatch 臂；构造与 `all_variants()` 均由 spec 表驱动
