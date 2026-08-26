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
│   ├── lib.rs               # Library root (8 public modules)
│   ├── commands/            # CLI arg definitions + all handlers
│   ├── web/                 # WebUI server (binary-only; axum routes, JobManager, embedded SPA assets)
│   ├── parser/              # Game data parsers (Lua, PO, recipes, prefab overrides)
│   │   └── prefab_override/ # Complex Lua AST walker (2657-line parser.rs)
│   ├── models/              # Data models (Recipe, PoEntry, TechReport)
│   │   └── recipe/          # Recipe sub-models (ingredient, options, prototyper, context)
│   ├── mapping/             # Data→wiki mapping framework (WikiMapper trait + builder)
│   │   └── mappers/         # Concrete mappers (PoEntryMapper, RecipeMapper)
│   ├── wiki/                # MediaWiki API client
│   ├── copyclip/            # Wiki module content updater (marker-based replacement)
│   ├── context.rs           # DstContext (zip archive, wiki client, env setup)
│   ├── error.rs             # Error enum (thiserror) + Result<T>
│   └── utils.rs             # diff_lines utility
├── examples/                # Test data files (.po, .lua, .json) + login.py
├── docs/                    # Design docs (PLAN.md, REFACTORING.md)
└── .github/workflows/      # CI (test+lint+build) + Release (4 cross-compile targets)
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Add a new CLI command | `src/commands/mod.rs` (enum) → `src/commands/maintain.rs` (handler) | Handlers are thin wrappers over `service::execute_job` |
| Add a new job type (CLI+Web) | `src/service/mod.rs` (`JobKind`) | Add variant + run fn; CLI wrapper comes free |
| Change WebUI behavior | `src/web/` | Binary-only module like commands; assets in `src/web/assets/` embedded via include_str! |
| Add a data browse endpoint | `src/web/api_data.rs` + `src/service/dataset.rs` | Dataset is cached per snapshot in memory |
| Parse a new Lua data type | `src/parser/` | Add new module, re-export in mod.rs |
| Add a new wiki data mapping | `src/mapping/mappers/` | Implement WikiMapper trait |
| Change wiki API interaction | `src/wiki/client.rs` | All HTTP/API logic here (throttle+retry in `send_with_retry`) |
| Change write/confirm policy | `src/service/mod.rs` (`WriteMode`, `decide_write`) | CLI maps --yes/--dry-run to WriteMode |
| Add a data model | `src/models/` | Add struct + serde derives |
| Update CopyClip (module constants) | `src/copyclip/` | TOML config in config.rs |
| Harvest the wiki corpus | `src/corpus/` + `service::JobKind::CorpusSync` | `corpus-fetch` CLI; layout/classifier per docs/WIKI_CORPUS_PLAN.md; output in gitignored `wikis/`; `corpus-index` rebuilds derived indexes (prefab registry / regions / facts) per docs/CORPUS_CODE_ATLAS_CONTRACT.md |
| Fix prefab name extraction | `src/parser/prefab_override/parser.rs` | 2657 lines, most complex file |
| Add environment config | `.env.example` → `.env` | HUIJI__* and DST__ROOT vars |

## CODE MAP

| Symbol | Type | Location | Role |
|--------|------|----------|------|
| `DstContext` | Struct | src/context.rs | App context: zip archive, wiki client, env vars |
| `Commands` | Enum | src/commands/mod.rs | 7 CLI subcommands (clap derive) |
| `WikiMapper` | Trait | src/mapping/mapper.rs | Core mapping interface: schema + rules + merge |
| `MappingBuilder<T>` | Struct | src/mapping/builder.rs | Fluent builder for field mappings + merge strategies |
| `WikiDataConverter` | Struct | src/mapping/converter.rs | Orchestrates mapping: parse → convert → compare → merge |
| `RecipeParser` | Struct | src/parser/recipe.rs | Parses Recipe{} calls from Lua via full_moon AST |
| `PrefabOverrideParser` | Struct | src/parser/prefab_override/parser.rs | Extracts prefab name overrides from Lua (factory patterns, control flow) |
| `LuaParser` | Struct | src/parser/lua.rs | Generic Lua variable/field location extraction |
| `PoParser` | Struct | src/parser/po.rs | Nom-based PO file parser |
| `WikiClient` | Struct | src/wiki/client.rs | MediaWiki API: login, get_page, edit_page, append/prepend |
| `CopyClipProcessor` | Struct | src/copyclip/mod.rs | Marker-based content replacement in wiki modules |
| `Recipe` | Struct | src/models/recipe/mod.rs | Game recipe: name, ingredients, tech, options |
| `PoEntry` / `PoFile` | Struct | src/models/po.rs | PO translation entry + file container |
| `TechReport` | Struct | src/models/tech_report.rs | Compares parsed vs wiki tech levels |
| `Error` | Enum | src/error.rs | 14 variants (Io, PoParse, Http, WikiApi, Zip, etc.) |

## CONVENTIONS
- **Edition 2021**, stable Rust only, no nightly features
- **No `rustfmt.toml`/`clippy.toml`** — all defaults. CI enforces `cargo fmt --check` + `cargo clippy -D warnings`
- **Inline tests only**: `#[cfg(test)] mod tests { use super::*; }` — no `tests/` directory, no dev-dependencies
- **No `unsafe`** anywhere in the codebase
- **Error handling**: `thiserror` derives → `crate::error::Result<T>` alias everywhere
- **Dependencies**: major version only (no pinning), semver-compatible ranges
- **`serde_json` with `preserve_order`** — JSON output maintains insertion order
- **`full_moon` with `lua52`** — Lua 5.2 dialect for DST game scripts
- **CLI args**: clap derive macros; `r#type` raw identifier for `--type` flag in copyclip

## ANTI-PATTERNS (THIS PROJECT)
- **DO NOT** add `rustfmt.toml` or `clippy.toml` — project uses Rust defaults
- **DO NOT** create `tests/` directory — inline `#[cfg(test)]` is the decided convention (see docs/REFACTORING.md)
- **DO NOT** use `unwrap()` outside tests — use `Result` propagation with `?`
- **DO NOT** add `unsafe` blocks — none exist, none should be needed
- **DO NOT** put Rust examples in `examples/` — it contains game data fixtures, not runnable examples

## UNIQUE STYLES
- `WikiMapper` trait: declarative mapping via `FieldMapping` enum (Direct/Transformed/Computed/Constant/Default/Ignored) + `MergeStrategy` (Overwrite/PreserveHistory/Merge/Custom)
- `MappingBuilder<T>`: fluent API with generics — field mapping rules composed via `.map_direct()`, `.map_transformed()`, `.map_computed()`, etc.
- `CopyClipProcessor`: marker-based content replacement (`--BEGIN/--END`) in wiki Lua modules
- `DstContext`: lazily opens ZIP archives of game scripts, provides unified access to PO files and Lua sources
- `RecipeParser`: handles Lua for-loops (numeric + generic/ipairs) to expand recipe definitions at parse time
- `PrefabOverrideParser`: deep AST walking across function boundaries, tracks local variables and parameters to resolve prefab name overrides

## COMMANDS
```bash
cargo build --release              # Build binary
cargo test                         # Run all 168 inline tests
cargo fmt --check                  # Check formatting
cargo clippy -- -D warnings        # Lint (CI uses this)
cargo run --release -- --help      # Show CLI help
```

## NOTES
- `.env` required for wiki operations (HUIJI__USERNAME, HUIJI__PASSWORD, HUIJI__X_AUTHKEY, DST__ROOT)
- WikiClient: global throttle (default 1 QPS, `WIKI__QPS`) + retry on 403/429/GET-5xx (`WIKI__MAX_RETRIES`); POST only retries WAF-level 403/429
- Edits carry `basetimestamp` + `assert=user`; conflicts surface as `Error::EditConflict`
- Batch APIs: `get_pages_meta` (≤50 titles), `page_exists`, `list_all_pages` (continuation-aware)
- Error variants RateLimited/PageNotFound/AuthExpired/EditConflict + `Error::is_retryable()` for automated branching
- Maintenance commands accept `--yes` (auto-confirm writes), `--dry-run` (no wiki writes, artifacts still written), `--report-json <path>` (machine-readable report); service-level enum is `service::WriteMode` with pure decision fn `decide_write`
- Structured logging: `cli_run` span (uuid run_id, command) wraps every invocation; `job` and `wiki_edit` (page, oldrevid/newrevid) spans inside service
- `examples/` contains game data (.po, .lua) + a Python login script, NOT Rust examples — `cargo run --example` finds nothing
- `src/commands/maintain.rs` is 743 lines with all 7+ command handlers — the largest non-parser file
- `src/parser/prefab_override/parser.rs` is 2657 lines — the most complex file in the project
- CI cross-compiles to 4 targets (Linux x86_64, macOS x86_64 + aarch64, Windows x86_64)
- Wiki API tests skip gracefully if `.env` not configured (won't fail in CI)
- `reqwest = "0.13"` — newer than typical 0.12.x; verify intentional
- No `rust-toolchain.toml` — CI uses `dtolnay/rust-toolchain@stable`
