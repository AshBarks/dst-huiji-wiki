# dst-anim-tool

Rust rewrite of [dont-starve-anim-tool](https://dont-starve-anim-tool.pages.dev/) — a CLI for extracting, splitting, and rendering Don't Starve Together (DST) animation files.

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
  extract  Extract raw files from .zip/.dyn
  split    Split atlas into individual sprite frame PNGs
  render   Render animation frames to PNG
  list     List available animations
  info     Show file metadata
```

### extract

Extract raw files (anim.bin, build.bin, .tex) from a .zip or .dyn archive.

```sh
dst-anim-tool extract <input> <output-dir>
```

- `.dyn` files are automatically XOR-decrypted before extraction
- All internal files are written to `output-dir/` preserving original filenames

```sh
dst-anim-tool extract data/anim/abigail_flower.zip output/abigail_flower
dst-anim-tool extract data/anim/dynamic/abigail_ice.dyn output/abigail_ice
```

### split

Decode atlas textures and split into per-symbol PNG frames.

```sh
dst-anim-tool split <input> <output-dir>
```

- Decodes KTEX textures (DXT1/3/5/RGBA/RGB)
- Crops sprite frames from atlas using build.bin vertex data
- Output structure: `output-dir/<symbol_name>/frame_N.png`

```sh
dst-anim-tool split data/anim/abigail_flower.zip output/split
```

Output example:

```
output/split/
  petal1/frame_0.png
  petal2/frame_0.png
  flower1/frame_0.png
  shdw/frame_0.png
  shdw/frame_1.png
  ...
```

### render

Render a specific animation as a sequence of composed PNG frames.

```sh
dst-anim-tool render <input> <bank_name>/<animation_name> <output-dir>
```

- Uses anim.bin elements + build.bin sprites + atlas textures
- Each frame is composited from multiple elements with transform matrices
- Output: `output-dir/frame_000.png`, `frame_001.png`, ...

Use `list` to discover available animation names.

```sh
dst-anim-tool render data/anim/abigail_flower.zip abigail_flower/idle_1 output/render
```

### list

List all available animations (bank_name/animation_name) in an archive.

```sh
dst-anim-tool list <input>
```

```sh
$ dst-anim-tool list data/anim/abigail_flower.zip
abigail_flower/haunted_pre
abigail_flower/haunted_pst
abigail_flower/idle_1
abigail_flower/idle_2
abigail_flower/idle_haunted_loop
```

### info

Show detailed metadata about an archive.

```sh
dst-anim-tool info <input>
```

Displays: anim version, banks, animations with frame counts, build symbols, atlas files, texture dimensions and pixel format.

## Supported File Types

| Type | Extension | Description |
|------|-----------|-------------|
| ZIP archive | `.zip` | Standard DST animation package |
| Dynamic skin | `.dyn` | XOR-encrypted ZIP (auto-decrypted) |

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
cargo test           # run tests
cargo clippy         # lint
cargo fmt            # format
```

Pre-commit order: `cargo fmt` -> `cargo clippy` -> `cargo test`
