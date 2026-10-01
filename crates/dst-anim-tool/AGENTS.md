# AGENTS.md

## Project

Rust rewrite of a JS-based Don't Starve Together (DST) animation file extraction tool. Workspace member crate `dst-anim-tool` (git subtree of the standalone repo). All planned features implemented — CLI (split/render/list/info/decrypt/decode/search/preview) and GUI preview with GIF/PNG export.

## Build & Verify

```
cargo build          # compile
cargo test           # run all tests (110 lib + 6 bin + 6 integration with local game data)
cargo clippy         # lint — run before committing
cargo fmt            # format — run before committing
cargo fmt -- --check # format check (non-destructive)
```

Order: `cargo fmt` → `cargo clippy` → `cargo test`

CI ran by the host workspace:

```
cargo test -p dst-anim-tool --no-default-features --features cli,gif
cargo test -p dst-anim-tool --no-default-features --features cli,gif -- --include-ignored  # local, needs data/anim symlink
```

## Architecture

See `docs/Plan.md` for phased development plan and `docs/Draft.md` for detailed implementation design. `docs/General.md` has the complete JS source logic reverse-engineering reference. `docs/OptimizationPlan.md` records the 2026-10 correctness/CLI/manifest optimization and its verification.

Modules (all in `src/`):

| File | Role |
|------|------|
| `main.rs` | Entry point, delegates to `cli::run()` |
| `cli.rs` | clap (derive) CLI: `split`, `render`, `list`, `info`, `decrypt`, `decode`, `search`, `preview`; render supports named `--anim/--out`, `--hide-layer`, `--disable-symbol`, `--frame-bounds rect\|ink`, `--manifest`/`--meta`, `--dump-elements`, `--dump-symbol`, `--strict`, `--jobs`; list/info support `--json`/`--allow-missing` |
| `reader.rs` | Binary reader — LE byte order, **jump reads** (offset-based reads for pre-scan passes in anim.bin/build.bin) |
| `specs.rs` | Magic constants (`ANIM/BILD/KTEX`), enums (`Platform/PixelFormat/TextureType/Direction`), direction suffix map via `LazyLock<HashMap>`, `KtexSpec` bit-field offsets, `detect_spec()` |
| `xor.rs` | XOR stream cipher — key `[141..148]`, permutation `[5,3,6,7,4,2,0,1]`, sequential block processing |
| `ktex.rs` | KTEX 兼容层 — `parse`/`decode_rgba` 委托给共享 crate `dst-ktex`（保留“仅尾部标记为真时反预乘”旧语义） |
| `anim.rs` | anim.bin parser — pre-scan to locate hash table, then full parse |
| `build_file.rs` | build.bin parser — two-pass: skip symbols → read verts → re-read symbols; `BuildSymbol.frame_for_anim_frame()` for duration-range frame lookup |
| `archive.rs` | .zip/.dyn dispatcher — .dyn detection (first 2 bytes != "PK"), XOR decrypt then unzip; `Arc<Vec<u8>>` shared tex data; `OnceCell`-cached `tex_files()`; `load_archive_files()` returns per-input archives for source-attributed `list`/`info` |
| `atlas.rs` | splitAtlas — UV→pixel crop, V-flip (`srcY = (1-maxV)*h`), 6-vert groups, pivot-centered paste; `compute_spans()` also yields per-sprite `InkRect` |
| `render.rs` | Frame composition — zIndex-sorted element overlay with 2×2 transform matrix; `RenderOptions` (disabled symbols/elements/layers, overrides, `BoundsKind::{Rect,Ink}`); `PreparedFrame` (+ `ink_bounds`) and `RenderedFrame.ink_bounds` (exact alpha bbox); `RenderDiagnostics` for missing symbol/frame/image reporting; `SymbolOverrideMap` + `parse_symbol_map()` for engine-style `OverrideSymbol` remapping (TOML `[[symbol_override]]` blocks) |
| `gif_export.rs` | GIF encoding — 6-bit color quantization with `QuantizeContext` (reusable `Box<[u16; 64³]>` lookup table), optional ffmpeg-based GIF from PNG sequence |
| `ui/mod.rs` | egui GUI — `App` struct, file loading, archive integration, export coordination |
| `ui/cache.rs` | Frame caching, background rendering, animation navigation |
| `ui/panels.rs` | egui panel layout — top/left/right/bottom bars, error overlay |
| `ui/export.rs` | Background GIF/PNG export thread logic |
| `error.rs` | thiserror error enum — `InvalidMagic`, `OutOfBounds`, `MissingCompanion`, `MissingData`, `Ktex`, `Other`, `Io`, `Zip`, `Gif`, `Ui` |

## Key Constraints

- **Edition 2021** — `edition.workspace = true`（宿主 workspace 统一 2021；无 let-chain，需嵌套 if）
- **No comments in code** unless explicitly asked
- **No emoji** unless explicitly asked
- **Do not commit** unless explicitly asked
- Test data lives in `data/` (symlinked to DST game files, gitignored)
- `refs/` contains original JS source (gitignored) — use for reverse-engineering constants
- `output/` is for CLI output (gitignored)
- `image` crate uses only `png` feature — no JPEG/WebP/TIFF etc.
- KTEX implementation lives in the shared `dst-ktex` crate (`crates/dst-ktex`), used by the host app too
- `zip` uses `default-features = false, features = ["deflate"]` — keeps wasm builds free of C compression backends
- Cargo features: `parallel` (rayon) is implied by `cli`; pure library consumers can enable `parallel` without clap/serde; `serde`/`serde_json` are optional under `cli`

## Implementation Gotchas

- `build_file.rs` (not `build.rs`) — avoids conflict with Cargo's build script convention
- anim.bin has a **pre-scan phase** that jumps through data via offset reads to locate the string hash table — the reader must support `read_at(offset)` style operations
- build.bin is **two-pass**: first pass skips symbol data to reach the vertex array, second pass re-reads symbols with vertex indices resolved
- KTEX spec bit field offsets differ between PreCave and PostCave — detection: `(specData >> 14) & 0x3FFFF == 0x3FFFF` means PreCave
- V coordinates in UV are **flipped** relative to pixel coordinates: `srcY = (1 - maxV) * height`
- `.dyn` files: if first 2 bytes are "PK" it's already decrypted (plain ZIP), otherwise apply XOR cipher first
- `render.rs` uses `prepare_animation_frames()` to pre-compute `ElementData` per frame, avoiding repeated `find_symbol_frame` lookups during rendering
- `archive.rs` stores tex data as `Arc<Vec<u8>>` — `tex_files()` uses `OnceCell` cache, only rebuilds on `merge()`
- GIF quantization uses `QuantizeContext` with `Box<[u16; 64³]>` lookup table, reused across frames
- `ktex::decode_rgba()` = `dst_ktex::decode_mipmap0` + optional demultiply; only demultiplies when the file tail marks pre-multiplied alpha
- `BuildSymbol.frame_for_anim_frame()` matches the JS `getFrame` semantics: a build frame covers anim frames `[frame_num, frame_num + duration - 1]` — exact match via `frame_index: HashMap<u32, usize>` first, then binary search (`partition_point`) on frames sorted by frame_num; discarding `duration` caused intermittent missing fx symbols (e.g. `p3_fx_ball_centre`)
- `Error` enum uses specific variants (`MissingCompanion`, `MissingData`, `Ktex`) instead of generic `UnknownFormat`
- UI background render / GIF export move `PreparedFrame` data to threads instead of cloning entire `BuildFile` + `AnimFile`
- `SymbolOverrideMap` mirrors the engine's `AnimState:OverrideSymbol`: anim placeholder symbols (`swap_object`, `fx_swap`) map to `(build_name, replacement_symbol)`; `build = ""` falls back to same-name lookup across the build_list; explicit build names restrict lookup to that build
- `render --build <file>...` loads extra build archives; anim-only inputs (e.g. `player_actions_axe.zip`) are valid when builds come from `--build`; named `--anim/--out` avoids the `--` separator that variadic `--build` otherwise forces before positionals
- `RenderOptions` owns and lowercases all disabled sets (symbols/elements/layers) in its `with_*` builders; `AnimElement.layer_lower` exists so layer matching stays case-insensitive (`AnimState:Hide` semantics)
- `PreparedFrame.ink_bounds` is a spans-based prediction: per-row span segments are transformed and unioned (exact-ish; rasterization tolerance ~1-2px), while `RenderedFrame.ink_bounds` scans the rendered alpha and is exact; `--frame-bounds ink` union keeps the existing 2px margin to cover the prediction tolerance
- `RenderDiagnostics` distinguishes `NoSymbol` / `NoFrame` / `NoImage` and records anim frame positions; `render --strict` turns any missing symbol into exit 1 while still writing output/manifest
- `list`/`info` use `load_archive_files()` (per-input) so multiple inputs no longer silently overwrite each other's anim/build; `render` still uses merged `load_archives()` for skin composition
- CLI `list`/`decode` exit non-zero on missing anim/tex unless `--allow-missing`; `info` only errors when the archive is completely empty (build-only packages stay exit 0)
- Data-dependent tests carry `#[ignore = "requires local DST game data (data/anim symlink)"]`; run them with `-- --include-ignored` after creating the symlink
