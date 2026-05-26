# AGENTS.md

## Project

Rust rewrite of a JS-based Don't Starve Together (DST) animation file extraction tool. Single crate `dst-anim-tool`. Currently in early implementation (empty `main.rs`, no source modules yet).

## Build & Verify

```
cargo build          # compile
cargo test           # run all tests (none yet, will be per-module #[test])
cargo clippy         # lint — run before committing
cargo fmt            # format — run before committing
cargo fmt -- --check # format check (non-destructive)
```

Order: `cargo fmt` → `cargo clippy` → `cargo test`

## Architecture

See `docs/Plan.md` for phased development plan and `docs/Draft.md` for detailed implementation design. `docs/General.md` has the complete JS source logic reverse-engineering reference.

Planned modules (all in `src/`):

| File | Role |
|------|------|
| `reader.rs` | Binary reader — LE byte order, **jump reads** (offset-based reads for pre-scan passes in anim.bin/build.bin) |
| `writer.rs` | Binary writer |
| `specs.rs` | Magic constants (`ANIM/BILD/KTEX`), enums (`Platform/PixelFormat/TextureType/Direction`), direction suffix map |
| `hash.rs` | DST string hash (djb2 variant: `(hash << 6) + (hash << 16) - hash`, lowercase input) |
| `xor.rs` | XOR stream cipher — key `[141..156]`, permutation `[5,3,6,7,4,2,0,1]`, sliding window |
| `ktex.rs` | KTEX texture — PreCave/PostCave spec bit fields, hand-rolled DXT1/3/5 → RGBA decode |
| `anim.rs` | anim.bin parser — pre-scan to locate hash table, then full parse |
| `build.rs` | build.bin parser — two-pass: skip symbols → read verts → re-read symbols |
| `archive.rs` | .zip/.dyn dispatcher — .dyn detection (first 2 bytes != "PK"), XOR decrypt then unzip |
| `atlas.rs` | splitAtlas — UV→pixel crop, V-flip (`srcY = (1-maxV)*h`), 6-vert groups, pivot-centered paste |
| `render.rs` | Frame composition — zIndex-sorted element overlay with 2×2 transform matrix |
| `cli.rs` | clap (derive) CLI: `extract`, `split`, `render` subcommands |
| `error.rs` | thiserror error enum |

## Key Constraints

- **Edition 2024** — requires rustc ≥ 1.85
- **No comments in code** unless explicitly asked
- **No emoji** unless explicitly asked
- **Do not commit** unless explicitly asked
- Test data lives in `data/` (symlinked to DST game files, gitignored) and `tests/data/`
- `refs/` contains original JS source (gitignored) — use for reverse-engineering constants
- `output/` is for CLI output (gitignored)

## Implementation Gotchas

- `build.rs` is a planned module name — if it conflicts with Cargo's build script convention, rename to `build_file.rs` or use `mod build_file;`
- anim.bin has a **pre-scan phase** that jumps through data via offset reads to locate the string hash table — the reader must support `read_at(offset)` style operations
- build.bin is **two-pass**: first pass skips symbol data to reach the vertex array, second pass re-reads symbols with vertex indices resolved
- KTEX spec bit field offsets differ between PreCave and PostCave — detection: `(specData >> 14) & 0x3FFFF == 0x3FFFF` means PreCave
- V coordinates in UV are **flipped** relative to pixel coordinates: `srcY = (1 - maxV) * height`
- `.dyn` files: if first 2 bytes are "PK" it's already decrypted (plain ZIP), otherwise apply XOR cipher first