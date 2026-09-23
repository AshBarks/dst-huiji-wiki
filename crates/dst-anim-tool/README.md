# dst-anim-tool

Rust rewrite of [dont-starve-anim-tool](https://dont-starve-anim-tool.pages.dev/) — a CLI and GUI tool for extracting, splitting, rendering, and previewing Don't Starve Together (DST) animation files.

## Requirements

- Rust >= 1.85 (edition 2024)

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
dst-anim-tool render -i <input> [--skin <skin-file>] [--build <extra-build>...] [--symbol-map <toml-file>] <bank_name>/<animation_name> <output-dir>
```

- Uses anim.bin elements + build.bin sprites + atlas textures
- Each frame is composited from multiple elements with transform matrices
- Output: `output-dir/frame_000.png`, `frame_001.png`, ...
- `--skin`: overlay a skin for alternate textures
- `--build`: load extra build archives (weapon/fx builds for symbol overrides)
- `--symbol-map`: TOML file mapping anim placeholder symbols to other builds' symbols, matching the engine's `AnimState:OverrideSymbol` behavior

Use `list` to discover available animation names.

```sh
dst-anim-tool render -i data/anim/abigail_flower.zip abigail_flower/idle_1 output/render
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
dst-anim-tool list -i <input>
```

```sh
$ dst-anim-tool list -i data/anim/abigail_flower.zip
abigail_flower/haunted_pre
abigail_flower/haunted_pst
abigail_flower/idle_1
abigail_flower/idle_2
abigail_flower/idle_haunted_loop
```

### info

Show detailed metadata about an archive.

```sh
dst-anim-tool info -i <input>
```

Displays: anim version, banks, animations with frame counts, build symbols, atlas files, texture dimensions and pixel format.

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

## Development

```sh
cargo build          # compile
cargo test           # run tests (107 tests)
cargo clippy         # lint
cargo fmt            # format
```

Pre-commit order: `cargo fmt` → `cargo clippy` → `cargo test`
