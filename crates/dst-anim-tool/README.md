# dst-anim-tool

Rust rewrite of [dont-starve-anim-tool](https://dont-starve-anim-tool.pages.dev/) — a CLI and GUI tool for extracting, splitting, rendering, and previewing Don't Starve Together (DST) animation files.

## Requirements

- Rust stable (edition 2021, shared with the workspace)

## Build

```sh
cargo build --release
```

Binary output: `target/release/dst-anim-tool`

## Usage

```
dst-anim-tool <COMMAND>

Commands:
  split    Split atlas into individual sprite frame PNGs
  render   Render animation frames to PNG
  list     List available animations
  info     Show file metadata
  decrypt  Decrypt .dyn to .zip
  decode   Decode .tex files to PNG
  search   Search a directory for a build symbol
  preview  Launch interactive GUI preview
```

### split

Decode atlas textures and split into per-symbol PNG frames.

```sh
dst-anim-tool split -i <input> [--skin <skin-file>] <output-dir>
```

- Decodes KTEX textures (DXT1/3/5/RGBA/RGB)
- Crops sprite frames from atlas using build.bin vertex data
- Output structure: `output-dir/<symbol_name>/frame_N.png`
- `--skin`: overlay a skin (.zip or .dyn with build + textures)

```sh
dst-anim-tool split -i data/anim/abigail_flower.zip output/split
```

### render

Render a specific animation as a sequence of composed PNG frames.

```sh
dst-anim-tool render -i <input>... [--skin <skin-file>] [--build <extra-build>...] [--symbol-map <toml-file>] \
  --anim <bank_name>/<animation_name> --out <output-dir> [options]
```

Positional `[-- <bank_name>/<animation_name> <output-dir>]` is still accepted for backward compatibility, but the named `--anim` / `--out` form avoids the `--` separator required by the variadic `--build`.

- Uses anim.bin elements + build.bin sprites + atlas textures
- Each frame is composited from multiple elements with transform matrices
- Output: `output-dir/frame_000.png`, `frame_001.png`, ... (`000` is the **source frame index**; frames that resolve to no elements are skipped, leaving gaps, and are listed in the manifest `skipped` array; beyond 999 frames lexical glob order no longer matches numeric order)
- `--skin`: overlay a skin for alternate textures
- `--build`: load extra build archives (weapon/fx builds for symbol overrides); repeat the flag or use `--build a.zip b.zip --` with positionals
- `--symbol-map`: TOML file mapping anim placeholder symbols to other builds' symbols, matching the engine's `AnimState:OverrideSymbol` behavior
- `--hide-layer <name>`: hide elements by layer name (case-insensitive), mirroring `AnimState:Hide("hat")`; repeatable
- `--disable-symbol <name>`: hide elements by symbol name (case-insensitive); repeatable
- `--frame-bounds rect|ink`: choose the canvas framing bounds. `rect` (default) = union of sprite rectangles (legacy behavior); `ink` = union of opaque-pixel bounds, much tighter. Per-frame `ink_bounds` in the manifest is always the exact rendered alpha bounding box
- `--manifest <file.json>` (alias `--meta`): write per-frame metadata (`frame_rate`, `src_index`, `file`, `bounds`, `ink_bounds`, `skipped`, `missing_symbols`). Use this instead of assuming 30 fps or glob-sorting the PNGs
- `--strict`: exit non-zero if any element could not be resolved to a build symbol/frame
- `--dump-symbol`: print all loaded build symbols as JSON
- `--dump-elements`: print per-frame anim elements (symbol/layer/matrix/translation) as JSON
- `--jobs <file.json>`: batch render many animations while loading/decoding the archives and atlases once

Missing symbols are always summarized on stderr, e.g. `missing symbol: beakfull (12 frames, layer=beak, kind=NoSymbol)`.

Use `list` to discover available animation names.

```sh
dst-anim-tool render -i data/anim/abigail_flower.zip --anim abigail_flower/idle_1 --out output/render
```

#### batch jobs file

```json
{
  "input": ["data/anim/ds_tallbird_basic.zip"],
  "out_dir": "output/batch",
  "frame_bounds": "ink",
  "jobs": [
    { "anim": "tallbird/idle_side" },
    { "anim": "tallbird/atk_side", "out": "output/atk", "manifest": "output/atk.json",
      "hide_layer": ["beakfull"], "disable_symbol": ["fx_shadow"] }
  ]
}
```

```sh
dst-anim-tool render --jobs jobs.json
```

#### symbol override map

Anim banks often reference placeholder symbols (e.g. `swap_object`, `fx_swap`) that
do not exist in any build; at runtime the game remaps them via
`AnimState:OverrideSymbol(anim_symbol, build_name, symbol_name)`. Pass a TOML file
to reproduce this:

```toml
[[symbol_override]]
symbol = "swap_object"        # symbol referenced by the anim elements
build = "swap_axe"            # build providing the replacement ("" = search all loaded builds)
replace_with = "swap_axe"     # replacement symbol name

[[symbol_override]]
symbol = "fx_swap"
build = "abigail_vial_fx"
replace_with = "fx_regen_02"
```

```sh
dst-anim-tool render -i data/anim/player_actions_axe.zip \
  --build data/anim/wilson.zip data/anim/swap_axe.zip \
  --symbol-map map.toml \
  -- wilson/chop_loop_side output/render
```

### list

List all available animations (bank_name/animation_name) in an archive.

```sh
dst-anim-tool list -i <input>... [--json] [--allow-missing]
```

- Without `--allow-missing`, a package with no anim.bin exits non-zero instead of silently succeeding
- `--json`: machine-readable output with `source_file`, `frame_rate` and frame counts per animation; each input is reported separately (no silent merge/overwrite)
- Text mode prints a `# <filename>:` header per input when more than one input is given

```sh
$ dst-anim-tool list -i data/anim/abigail_flower.zip
abigail_flower/haunted_pre
abigail_flower/haunted_pst
abigail_flower/idle_1
abigail_flower/idle_2
abigail_flower/haunted_loop
```

### info

Show detailed metadata about an archive.

```sh
dst-anim-tool info -i <input>... [--json] [--allow-missing]
```

Displays: anim version, banks, animations with frame counts, build symbols, atlas files, texture dimensions and compression format. `--json` adds `source_file` per input and structured build/tex fields. A build-only package is still valid input (exit 0); only a completely empty archive is an error unless `--allow-missing` is passed.

### decrypt

Decrypt a .dyn file to a plain .zip.

```sh
dst-anim-tool decrypt <input.dyn> <output.zip>
```

### decode

Decode all .tex files in an archive to PNG.

```sh
dst-anim-tool decode <input> <output-dir>
```

### search

Recursively search a directory for archives containing a build symbol.

```sh
dst-anim-tool search -d <dir> <symbol> [--case-sensitive]
```

### preview

Launch an interactive GUI for browsing and previewing animations.

```sh
dst-anim-tool preview [-i <input-files>...]
```

Features:
- Drag-and-drop multi-archive loading
- Animation/bank/frame tree navigation
- Per-element visibility toggling with live preview
- Playback with adjustable speed
- Frame-by-frame stepping
- PNG export of current frame
- GIF export (builtin quantizer or ffmpeg-based for better quality)
- Multi-build layering with atlas assignment
- Background frame pre-rendering

> **Note (Linux/Wayland)**: File drag-and-drop relies on the underlying windowing library (winit). winit currently does **not** implement file drag-and-drop reception on Wayland sessions (see [rust-windowing/winit#1881](https://github.com/rust-windowing/winit/issues/1881), long-standing), so dragging files onto the window has no effect under native Wayland. Alternatives:
> - Click the **Open File** button to pick files
> - Pass files via CLI: `dst-anim-tool preview <files...>`
> - Run under XWayland (temporary workaround): `env -u WAYLAND_DISPLAY dst-anim-tool preview`
>
> Drag-and-drop works normally on X11 / XWayland / Windows / macOS.

## Supported File Types

| Type | Extension | Description |
|------|-----------|-------------|
| ZIP archive | `.zip` | Standard DST animation package |
| Dynamic skin | `.dyn` | XOR-encrypted ZIP (auto-decrypted) |
| Binary | `.bin` | Standalone anim.bin or build.bin |

## Supported Texture Formats

| Format | Description |
|--------|-------------|
| DXT1 | S3TC compressed, 1-bit alpha |
| DXT3 | S3TC compressed, explicit alpha |
| DXT5 | S3TC compressed, interpolated alpha |
| RGBA | Uncompressed RGBA |
| RGB | Uncompressed RGB |

## Feature Flags

| Feature | Default | Enables |
|---------|---------|---------|
| `cli` | yes | binary, JSON output (`serde`/`serde_json`), implies `parallel` |
| `gui` | yes | `preview` GUI (eframe/egui/rfd), implies `cli` |
| `gif` | yes | GIF encoding (`gif`, `which`/ffmpeg) |
| `parallel` | via `cli` | rayon-based `render_frame_with_elements_par` |

Pure library consumers can use `default-features = false` for the serial render path, or `default-features = false, features = ["parallel"]` to get rayon without clap/serde.

```sh
cargo check --no-default-features
cargo check --no-default-features --features parallel
```

## wasm Support

There is no wasm CI guarantee for the full tool, but the byte-oriented library paths compile for `wasm32-unknown-unknown`:

```sh
cargo check --target wasm32-unknown-unknown --no-default-features
```

- Usable on wasm: `parse_zip`, `parse_dyn`, `parse_anim_bin`, `parse_build_bin`, `ktex`, `anim`, `build_file`, `atlas`, and the serial `render` path (no rayon, no `std::fs`).
- Not available on wasm: `load_archives`/`load_archive_files` (use `std::fs`; feed bytes to `parse_zip` instead), `parallel`, `cli`, and `gui`.
- `zip` is configured with `default-features = false, features = ["deflate"]` so no C compression backends are required.

## Development

```sh
cargo build          # compile
cargo test           # run tests (110 lib + 6 bin + 6 integration with local game data)
cargo clippy         # lint
cargo fmt            # format
```

Pre-commit order: `cargo fmt` → `cargo clippy` → `cargo test`
