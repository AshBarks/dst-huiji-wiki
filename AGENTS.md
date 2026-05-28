# AGENTS.md

## Project

Rust rewrite of a JS-based Don't Starve Together (DST) animation file extraction tool. Single crate `dst-anim-tool`. All planned features implemented — CLI (extract/split/render/list/info/decrypt/decode/preview) and GUI preview with GIF/PNG export.

## Build & Verify

```
cargo build          # compile
cargo test           # run all tests (63 per-module #[test])
cargo clippy         # lint — run before committing
cargo fmt            # format — run before committing
cargo fmt -- --check # format check (non-destructive)
```

Order: `cargo fmt` → `cargo clippy` → `cargo test`

## Architecture

See `docs/Plan.md` for phased development plan and `docs/Draft.md` for detailed implementation design. `docs/General.md` has the complete JS source logic reverse-engineering reference.

Modules (all in `src/`):

| File | Role |
|------|------|
| `main.rs` | Entry point, delegates to `cli::run()` |
| `cli.rs` | clap (derive) CLI: `extract`, `split`, `render`, `list`, `info`, `decrypt`, `decode`, `preview` subcommands |
| `reader.rs` | Binary reader — LE byte order, **jump reads** (offset-based reads for pre-scan passes in anim.bin/build.bin) |
| `writer.rs` | Binary writer — LE byte order, used by anim/bin write functions |
| `specs.rs` | Magic constants (`ANIM/BILD/KTEX`), enums (`Platform/PixelFormat/TextureType/Direction`), direction suffix map via `LazyLock<HashMap>`, `KtexSpec` bit-field offsets, `detect_spec()` |
| `hash.rs` | DST string hash (djb2 variant: `(hash << 6) + (hash << 16) - hash`, byte-level lowercase input) |
| `xor.rs` | XOR stream cipher — key `[141..148]`, permutation `[5,3,6,7,4,2,0,1]`, sequential block processing |
| `ktex.rs` | KTEX texture — PreCave/PostCave spec bit fields, hand-rolled DXT1/3/5 → RGBA decode with bulk row copy for full blocks |
| `anim.rs` | anim.bin parser — pre-scan to locate hash table, then full parse; also `write_anim()` for serialization |
| `build_file.rs` | build.bin parser — two-pass: skip symbols → read verts → re-read symbols; also `write_build()` |
| `archive.rs` | .zip/.dyn dispatcher — .dyn detection (first 2 bytes != "PK"), XOR decrypt then unzip; `Arc<Vec<u8>>` shared tex data |
| `atlas.rs` | splitAtlas — UV→pixel crop, V-flip (`srcY = (1-maxV)*h`), 6-vert groups, pivot-centered paste |
| `render.rs` | Frame composition — zIndex-sorted element overlay with 2×2 transform matrix; pre-computed `ElementData` + `PreparedFrame` for batch rendering |
| `gif_export.rs` | GIF encoding — 6-bit color quantization with direct array lookup table, optional ffmpeg-based GIF from PNG sequence |
| `ui.rs` | egui GUI — multi-archive drag-and-drop, animation/bank/frame tree navigation, background frame rendering, GIF/PNG export with rayon |
| `error.rs` | thiserror error enum |

## Key Constraints

- **Edition 2024** — requires rustc ≥ 1.85
- **No comments in code** unless explicitly asked
- **No emoji** unless explicitly asked
- **Do not commit** unless explicitly asked
- Test data lives in `data/` (symlinked to DST game files, gitignored) and `tests/data/`
- `refs/` contains original JS source (gitignored) — use for reverse-engineering constants
- `output/` is for CLI output (gitignored)
- `image` crate uses only `png` feature — no JPEG/WebP/TIFF etc.

## Implementation Gotchas

- `build_file.rs` (not `build.rs`) — avoids conflict with Cargo's build script convention
- anim.bin has a **pre-scan phase** that jumps through data via offset reads to locate the string hash table — the reader must support `read_at(offset)` style operations
- build.bin is **two-pass**: first pass skips symbol data to reach the vertex array, second pass re-reads symbols with vertex indices resolved
- KTEX spec bit field offsets differ between PreCave and PostCave — detection: `(specData >> 14) & 0x3FFFF == 0x3FFFF` means PreCave
- V coordinates in UV are **flipped** relative to pixel coordinates: `srcY = (1 - maxV) * height`
- `.dyn` files: if first 2 bytes are "PK" it's already decrypted (plain ZIP), otherwise apply XOR cipher first
- `render.rs` uses `prepare_animation_frames()` to pre-compute `ElementData` per frame, avoiding repeated `find_symbol_frame` lookups during rendering
- `archive.rs` stores tex data as `Arc<Vec<u8>>` — `tex_files()` only increments ref counts, no deep copies
- DXT decode uses bulk 16-byte row copy for complete (non-edge) blocks, falls back to per-pixel for edge blocks
- `flip_y` uses `copy_from_slice` + `copy_within` for row-level swap instead of per-byte swap
- `un_premultiply_alpha` uses integer `div_ceil()` instead of float division
- GIF quantization uses a `Vec<u16>` (64³ entries) direct-index table instead of `HashMap<[u8;3], u8>`
- UI background render / GIF export move `PreparedFrame` data to threads instead of cloning entire `BuildFile` + `AnimFile`
