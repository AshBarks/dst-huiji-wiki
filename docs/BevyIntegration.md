# Bevy Integration Guide

Integrating DST animation assets into the [Bevy](https://bevyengine.org/) game engine.

DST animations are **layered cutout 2D animations** — each frame is composed of multiple independent elements, each with its own texture atlas region, 2×2 affine transform matrix (rotation/scale/shear), z-index for draw order, and pivot offset. This is fundamentally different from simple sprite-sheet animation and requires a custom ECS-driven animation system rather than Bevy's built-in `AnimationPlayer`.

---

## Table of Contents

1. [Data Pipeline Mapping](#1-data-pipeline-mapping)
2. [Architecture Overview](#2-architecture-overview)
3. [Custom AssetLoader](#3-custom-assetloader)
4. [Texture Creation from KTEX](#4-texture-creation-from-ktex)
5. [TextureAtlasLayout — Keeping Atlases Whole](#5-textureatlaslayout--keeping-atlases-whole)
6. [2×2 Transform Matrix → Bevy Transform](#6-2×2-transform-matrix--bevy-transform)
7. [Frame Advancement System](#7-frame-advancement-system)
8. [Z-Ordering](#8-z-ordering)
9. [Multi-Build Layering (Skins)](#9-multi-build-layering-skins)
10. [Coordinate System Notes](#10-coordinate-system-notes)
11. [Plugin Registration](#11-plugin-registration)
12. [Usage Example](#12-usage-example)
13. [Appendix: DST Data Structures Reference](#appendix-dst-data-structures-reference)

---

## 1. Data Pipeline Mapping

The existing rendering pipeline:

```
.dyn/.zip ──→ XOR decrypt ──→ unzip ──→ anim.bin + build.bin + .tex
                                                     │
                            ┌────────────────────────┼────────────────────────┐
                            ▼                        ▼                        ▼
                       AnimFile                 BuildFile                KTEX textures
                    (frames/elements)        (symbols/verts/UV)        (DXT→RGBA decode)
                            │                        │                        │
                            └──── cross-reference ───┘                        │
                                       │                                       │
                                       ▼                                       ▼
                                 ElementData                              split_atlas()
                               (sprite + matrix)                         (UV→pixel crop)
                                       │                                       │
                                       └──────────────┬────────────────────────┘
                                                      ▼
                                              PreparedFrame
                                           (precomputed frame data)
                                                      │
                                                      ▼
                                   render_frame_with_elements()
                                (z-sort + affine transform + alpha blend)
```

**Key decision for Bevy**: Skip `split_atlas()` — do not pre-crop each BuildFrame into an individual `RgbaImage`. Instead, keep atlas textures whole and use `TextureAtlasLayout` to manage UV regions. This is more GPU-friendly and more idiomatic in Bevy.

---

## 2. Architecture Overview

```
┌───────────────────────────────────────────────────────────────┐
│                      DstAnimationPlugin                        │
│                                                                │
│  Asset Layer:                                                  │
│    DstAnimLoader (impl AssetLoader)                            │
│      .dyn/.zip → XOR decrypt → unzip → parse → DstAnimAsset   │
│                                                                │
│    DstAnimAsset (Asset)                                        │
│      ├── atlas_textures: Vec<Handle<Image>>          KTEX→RGBA│
│      ├── atlas_layouts: Vec<Handle<TextureAtlasLayout>>       │
│      ├── anim: AnimFile                                       │
│      ├── build: BuildFile                                     │
│      └── frame_elements: Vec<Vec<DstElementPose>>  precomputed│
│                                                                │
│  Component Layer:                                              │
│    DstAnimator                                                 │
│      ├── asset: Handle<DstAnimAsset>                           │
│      ├── bank_name / anim_name: String                         │
│      ├── frame_index: usize                                    │
│      ├── timer: Timer                                          │
│      ├── playing: bool                                         │
│      └── disabled_elements: HashSet<(String, String)>          │
│                                                                │
│  System Layer:                                                 │
│    spawn_dst_character   — spawn child entities on first load  │
│    advance_dst_animation — timer-driven frame advance          │
│    update_dst_elements   — write Sprite.index + Transform      │
│                                                                │
│  Marker Component:                                             │
│    DstElementChild — marks per-element child entities           │
└───────────────────────────────────────────────────────────────┘
```

### Why not Bevy's built-in AnimationPlayer?

| DST Requirement | Bevy AnimationPlayer | Custom ECS |
|---|---|---|
| Per-element 2×2 affine matrix (may include shear) | No shear support in Transform | Trivial with custom mesh or Affine3A |
| Discrete frame stepping | Awkward (Step interpolation) | Natural (Timer + index) |
| Per-element z-index | Possible but awkward | Natural (translation.z) |
| Frame-based (not keyframe) | Designed for property keyframes | Timer.tick() is simpler |
| Blend between clips | Built-in | Manual (if needed later) |
| Timeline scrubbing | Built-in | Manual (if needed later) |

The built-in system is designed for property keyframe animation (3D transforms, material properties). DST's frame-based, per-element composition model is better served by a custom system.

---

## 3. Custom AssetLoader

### DstAnimAsset Definition

```rust
use bevy::prelude::*;
use bevy::render::texture::{Extent3d, TextureDimension, TextureFormat};
use bevy::render::render_asset::RenderAssetUsages;

#[derive(Asset, TypePath)]
struct DstAnimAsset {
    atlas_textures: Vec<Handle<Image>>,
    atlas_layouts: Vec<Handle<TextureAtlasLayout>>,
    anim: AnimFile,
    build: BuildFile,
    frame_elements: Vec<Vec<DstElementPose>>,
}

struct DstElementPose {
    atlas_index: usize,
    rect_index: usize,
    matrix: Mat2,
    offset: Vec2,
    z_index: f32,
}
```

### AssetLoader Implementation

```rust
use bevy::asset::{io::Reader, AssetLoader, LoadContext};
use thiserror::Error;

#[derive(Default)]
struct DstAnimLoader;

#[derive(Error, Debug)]
enum DstAnimLoadError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Missing texture: {0}")]
    MissingTexture(String),
    #[error("Invalid magic: {0:#x}")]
    InvalidMagic(u32),
    #[error("Parse error: {0}")]
    Parse(String),
}

impl AssetLoader for DstAnimLoader {
    type Asset = DstAnimAsset;
    type Settings = ();
    type Error = DstAnimLoadError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;

        // Step 1: Parse archive (reuses existing archive.rs logic)
        // - .dyn detection: first 2 bytes != "PK" → XOR decrypt
        // - Then unzip to extract anim.bin, build.bin, .tex files
        let archive = parse_archive_from_bytes(&bytes)
            .map_err(|e| DstAnimLoadError::Parse(e.to_string()))?;

        let anim = archive.anim.ok_or_else(|| {
            DstAnimLoadError::Parse("missing anim.bin".into())
        })?;
        let build = archive.build.ok_or_else(|| {
            DstAnimLoadError::Parse("missing build.bin".into())
        })?;
        let tex_files = archive.tex_files();

        // Step 2: Decode KTEX textures → Bevy Image + TextureAtlasLayout
        let mut atlas_textures = Vec::new();
        let mut atlas_layouts = Vec::new();

        for atlas_ref in &build.atlases {
            let tex_data = tex_files.get(&atlas_ref.name)
                .ok_or_else(|| DstAnimLoadError::MissingTexture(atlas_ref.name.clone()))?;

            // Decode KTEX → RGBA (reuses ktex.rs, which already does
            // flip_y() and un_premultiply_alpha())
            let ktex = parse_ktex(tex_data)
                .map_err(|e| DstAnimLoadError::Parse(e.to_string()))?;
            let rgba_image = ktex.to_image_rgba();
            let (w, h) = (rgba_image.width(), rgba_image.height());

            // Create Bevy Image from raw RGBA bytes
            let image = Image::new(
                Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                rgba_image.into_raw(),
                TextureFormat::Rgba8UnormSrgb,
                RenderAssetUsages::RENDER_WORLD,
            );
            let image_handle = load_context.labeled_asset_scope(
                atlas_ref.name.clone(),
                |_: &mut LoadContext| image,
            );

            // Build TextureAtlasLayout from BuildFrame UV coordinates
            let mut layout = TextureAtlasLayout::new_empty(UVec2::new(w, h));
            for symbol in &build.symbols {
                for frame in &symbol.frames {
                    let (min_u, max_u, min_v, max_v) = calc_uv_bounds(&frame.verts);
                    layout.add_texture(URect {
                        min: UVec2::new(
                            (min_u * w as f32).round() as u32,
                            ((1.0 - max_v) * h as f32).round() as u32,
                        ),
                        max: UVec2::new(
                            (max_u * w as f32).round() as u32,
                            ((1.0 - min_v) * h as f32).round() as u32,
                        ),
                    });
                }
            }
            let layout_handle = load_context.labeled_asset_scope(
                format!("layout_{}", atlas_ref.name),
                |_: &mut LoadContext| layout,
            );

            atlas_textures.push(image_handle);
            atlas_layouts.push(layout_handle);
        }

        // Step 3: Precompute per-frame element poses
        let frame_elements = precompute_frame_elements(&anim, &build, &atlas_layouts);

        Ok(DstAnimAsset {
            atlas_textures,
            atlas_layouts,
            anim,
            build,
            frame_elements,
        })
    }

    fn extensions(&self) -> &[&str] {
        &["dyn", "zip"]
    }
}
```

### UV Bounds Calculation

Reuses the same logic as `atlas.rs::calc_uv_bounds()`:

```rust
fn calc_uv_bounds(verts: &[BuildVert]) -> (f32, f32, f32, f32) {
    // 6 vertices per quad = 2 triangles
    // verts[0] and verts[1] contain the min/max corners
    let min_u = verts.iter().map(|v| v.u).fold(f32::MAX, f32::min);
    let max_u = verts.iter().map(|v| v.u).fold(f32::MIN, f32::max);
    let min_v = verts.iter().map(|v| v.v).fold(f32::MAX, f32::min);
    let max_v = verts.iter().map(|v| v.v).fold(f32::MIN, f32::max);
    (min_u, max_u, min_v, max_v)
}
```

### Precomputing Frame Elements

This is the Bevy equivalent of `render.rs::prepare_animation_frames()`. It resolves all symbol lookups once at load time:

```rust
fn precompute_frame_elements(
    anim: &AnimFile,
    build: &BuildFile,
    atlas_layouts: &[Handle<TextureAtlasLayout>],
) -> Vec<Vec<DstElementPose>> {
    let mut all_frames = Vec::new();

    for bank in &anim.banks {
        for animation in &bank.animations {
            for frame in &animation.frames {
                let mut elements = Vec::new();

                for elem in &frame.elements {
                    // Look up BuildSymbol by name (case-insensitive)
                    let symbol_key = &elem.symbol_lower;
                    let sym_idx = match build.symbol_index.get(symbol_key) {
                        Some(&idx) => idx,
                        None => continue,
                    };
                    let symbol = &build.symbols[sym_idx];

                    // Look up BuildFrame by frame number (O(1) via frame_index)
                    let bf_idx = match symbol.frame_index.get(&elem.frame_num) {
                        Some(&idx) => idx,
                        None => continue,
                    };
                    let bf = &symbol.frames[bf_idx];

                    // Determine atlas index from BuildVert.w
                    let atlas_idx = bf.verts.first().map(|v| v as u32).unwrap_or(0) as usize;

                    // Determine rect index in TextureAtlasLayout
                    // (computed during layout construction, stored in a lookup map)
                    let rect_index = /* index from BuildFrame → layout rect mapping */;

                    // 2×2 transform matrix: row-major [a,b; c,d] → column-major Mat2
                    let matrix = Mat2::from_cols(
                        Vec2::new(elem.a, elem.c),
                        Vec2::new(elem.b, elem.d),
                    );

                    elements.push(DstElementPose {
                        atlas_index: atlas_idx,
                        rect_index,
                        matrix,
                        offset: Vec2::new(elem.tx, elem.ty),
                        z_index: elem.z_index,
                    });
                }

                all_frames.push(elements);
            }
        }
    }

    all_frames
}
```

---

## 4. Texture Creation from KTEX

KTEX textures decode to raw RGBA bytes. The existing `ktex.rs` already handles:

- DXT1/DXT3/DXT5 block decompression
- `un_premultiply_alpha()` (using integer `div_ceil()`)
- `flip_y()` (row-level swap)

So the output `RgbaImage` is ready for Bevy as-is:

```rust
fn rgba_image_to_bevy(rgba: &RgbaImage) -> Image {
    Image::new(
        Extent3d {
            width: rgba.width(),
            height: rgba.height(),
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        rgba.clone().into_raw(),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}
```

**Texture format**: Use `Rgba8UnormSrgb` — Bevy's sprite pipeline expects sRGB for correct color rendering. The `Rgba8Unorm` variant would produce washed-out colors.

**Future optimization**: If you want to skip CPU-side DXT decompression, you could upload the raw DXT blocks as `Bc1RgbaUnorm` / `Bc2RgbaUnorm` / `Bc3RgbaUnorm` (the GPU-native equivalents of DXT1/DXT3/DXT5). However, this requires the texture dimensions to be multiples of 4 and the `wgpu` feature `texture-compression-bc` to be enabled. The current CPU-decode approach is simpler and works universally.

---

## 5. TextureAtlasLayout — Keeping Atlases Whole

### Why Skip split_atlas()?

The current `split_atlas()` pre-crops each BuildFrame into an individual `RgbaImage`. This works for the CLI/GUI tool but is suboptimal for Bevy because:

1. **GPU memory**: Thousands of small textures waste GPU memory (each has its own allocation, padding, and mip chain overhead)
2. **Draw calls**: Each unique texture = a separate draw call. One atlas = one draw call per batch
3. **Bevy idiom**: `TextureAtlasLayout` is the standard way to handle sprite regions in Bevy

### Building the Layout

```rust
fn build_atlas_layout(build: &BuildFile, atlas_width: u32, atlas_height: u32) -> TextureAtlasLayout {
    let mut layout = TextureAtlasLayout::new_empty(UVec2::new(atlas_width, atlas_height));
    let w = atlas_width as f32;
    let h = atlas_height as f32;

    for symbol in &build.symbols {
        for frame in &symbol.frames {
            let (min_u, max_u, min_v, max_v) = calc_uv_bounds(&frame.verts);

            // V-flip: DST uses V=0 at bottom, Bevy uses V=0 at top
            // Since ktex.rs already does flip_y(), the texture is uploaded
            // with the correct orientation. The pixel coordinates here match
            // the flipped texture.
            layout.add_texture(URect {
                min: UVec2::new(
                    (min_u * w).round() as u32,
                    ((1.0 - max_v) * h).round() as u32,
                ),
                max: UVec2::new(
                    (max_u * w).round() as u32,
                    ((1.0 - min_v) * h).round() as u32,
                ),
            });
        }
    }

    layout
}
```

### V-Flip Detail

DST's UV coordinates have V=0 at the bottom and V=1 at the top. In `atlas.rs`, the pixel coordinate conversion is:

```
srcY = (1 - maxV) * height
```

Since `ktex.rs::to_image_rgba()` already calls `flip_y()` on the decoded pixel data, the uploaded Bevy texture has V=0 at the top (standard OpenGL convention). The `URect` pixel coordinates must account for this — the formula above does so correctly.

---

## 6. 2×2 Transform Matrix → Bevy Transform

DST's per-element transform is a 2D affine matrix stored as `[a, b, c, d] + [tx, ty]`:

```
[a  b]   [tx]
[c  d] + [ty]
```

Applied to sprite pixels:
```
out_x = tx + local_x * a + local_y * c
out_y = ty + local_x * b + local_y * d
```

Bevy's `Transform` decomposes into `scale + rotation + translation`, which **does not support shear**. DST matrices may include shear. Three approaches:

### Approach A: Decompose to Scale + Rotation (No Shear)

If shear is rare in DST animations, decompose the matrix and accept the loss:

```rust
fn dst_matrix_to_transform(
    a: f32, b: f32, c: f32, d: f32,
    tx: f32, ty: f32, z: f32,
) -> Transform {
    // Row-major [a b; c d] → Bevy column-major Mat2
    let mat = Mat2::from_cols(Vec2::new(a, c), Vec2::new(b, d));

    let scale_x = mat.x_axis.length();
    let scale_y = mat.y_axis.length();
    let angle = mat.y_axis.x.atan2(mat.y_axis.y);

    Transform {
        translation: Vec3::new(tx, ty, z * 0.001),
        rotation: Quat::from_rotation_z(angle),
        scale: Vec3::new(scale_x, scale_y, 1.0),
    }
}
```

**Caveat**: If the matrix contains shear, this decomposition will be lossy. The visual result may differ from the original DST rendering.

### Approach B: Custom Mesh2d with Baked Transforms (Full Fidelity)

For full fidelity including shear, generate a custom `Mesh` per element with the transform baked into vertex positions. This is the approach used by `bevy_animate_atlas`:

```rust
fn build_element_mesh(
    uv_rect: URect,
    atlas_size: UVec2,
    matrix: Mat2,
    offset: Vec2,
    z: f32,
) -> Mesh {
    let w = atlas_size.x as f32;
    let h = atlas_size.y as f32;
    let u_min = uv_rect.min.x as f32 / w;
    let u_max = uv_rect.max.x as f32 / w;
    let v_min = uv_rect.min.y as f32 / h;
    let v_max = uv_rect.max.y as f32 / h;

    let sprite_w = uv_rect.width() as f32;
    let sprite_h = uv_rect.height() as f32;

    // Four corners in local space, then transform
    let corners_local = [
        Vec2::new(0.0, 0.0),
        Vec2::new(sprite_w, 0.0),
        Vec2::new(sprite_w, sprite_h),
        Vec2::new(0.0, sprite_h),
    ];

    // Apply 2×2 matrix + translation
    let positions: Vec<[f32; 3]> = corners_local
        .iter()
        .map(|c| {
            let t = matrix * *c + offset;
            [t.x, -t.y, z * 0.001]
        })
        .collect();

    let uvs: Vec<[f32; 2]> = vec![
        [u_min, v_max],
        [u_max, v_max],
        [u_max, v_min],
        [u_min, v_min],
    ];

    let indices: Vec<u32> = vec![0, 1, 2, 0, 2, 3];

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, Default::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}
```

**Tradeoff**: Higher fidelity but more complex — requires `Mesh2d` + `MeshMaterial2d<ColorMaterial>` components instead of simple `Sprite`. Also requires rebuilding the mesh each frame (or at least the vertex buffer).

### Approach C: Hybrid — Decompose When Possible, Mesh When Needed

Check if the matrix has shear at runtime:

```rust
fn has_shear(a: f32, b: f32, c: f32, d: f32) -> bool {
    // A pure rotation+scale matrix has perpendicular columns:
    // col1 · col2 = 0
    let dot = a * b + c * d;
    dot.abs() > 0.01 // threshold for "no shear"
}
```

If no shear → Approach A (Sprite + Transform). If shear → Approach B (custom Mesh). This gives the best of both worlds but adds complexity.

### Recommendation

Start with **Approach A**. DST animations in practice rarely use shear — the 2×2 matrix is typically just rotation + uniform/non-uniform scale. If visual artifacts appear on specific animations, fall back to Approach B for those cases.

---

## 7. Frame Advancement System

### DstAnimator Component

```rust
#[derive(Component)]
struct DstAnimator {
    asset: Handle<DstAnimAsset>,
    bank_name: String,
    anim_name: String,
    frame_index: usize,
    timer: Timer,
    playing: bool,
}

#[derive(Component)]
struct DstElementChild {
    element_index: usize,
}
```

### Spawning the Character

```rust
fn spawn_dst_character(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    let handle = asset_server.load("characters/wilson.dyn");

    commands.spawn((
        DstAnimator {
            asset: handle,
            bank_name: "wilson".into(),
            anim_name: "idle".into(),
            frame_index: 0,
            timer: Timer::from_seconds(1.0 / 30.0, TimerMode::Repeating),
            playing: true,
        },
        Transform::default(),
        Visibility::default(),
    ));
}
```

### System: Initialize Child Entities

Once the `DstAnimAsset` is loaded, spawn child entities for each element:

```rust
fn init_dst_elements(
    mut commands: Commands,
    assets: Res<Assets<DstAnimAsset>>,
    animators: Query<(Entity, &DstAnimator), Added<DstAnimator>>,
) {
    for (entity, animator) in &animators {
        let asset = match assets.get(&animator.asset) {
            Some(a) => a,
            None => continue,
        };

        // Find the max element count across all frames
        // (to pre-spawn enough child entities)
        let max_elements = asset.frame_elements
            .iter()
            .map(|frame| frame.len())
            .max()
            .unwrap_or(0);

        commands.entity(entity).with_children(|parent| {
            for i in 0..max_elements {
                parent.spawn((
                    DstElementChild { element_index: i },
                    Sprite::default(),
                    Transform::default(),
                    Visibility::Hidden, // hidden until first frame update
                ));
            }
        });
    }
}
```

### System: Advance Animation + Update Elements

```rust
fn update_dst_animation(
    time: Res<Time>,
    assets: Res<Assets<DstAnimAsset>>,
    mut animators: Query<(Entity, &mut DstAnimator)>,
    children_query: Query<&Children>,
    mut element_query: Query<(&DstElementChild, &mut Sprite, &mut Transform, &mut Visibility)>,
) {
    for (entity, mut animator) in &mut animators {
        if !animator.playing {
            continue;
        }

        animator.timer.tick(time.delta());
        if !animator.timer.just_finished() {
            continue;
        }

        let asset = match assets.get(&animator.asset) {
            Some(a) => a,
            None => continue,
        };

        // Advance frame index
        let frame_count = get_frame_count(
            &asset.anim,
            &animator.bank_name,
            &animator.anim_name,
        );
        animator.frame_index = (animator.frame_index + 1) % frame_count;

        // Get element poses for current frame
        let elements = &asset.frame_elements[animator.frame_index];

        // Update child entities
        let Ok(children) = children_query.get(entity) else {
            continue;
        };

        for child in children.iter() {
            let Ok((elem_child, mut sprite, mut transform, mut visibility)) =
                element_query.get_mut(*child)
            else {
                continue;
            };

            let i = elem_child.element_index;

            if let Some(pose) = elements.get(i) {
                // Show element
                *visibility = Visibility::Visible;

                // Update texture atlas index
                if let Some(atlas) = &mut sprite.texture_atlas {
                    atlas.index = pose.rect_index;
                }

                // Apply 2×2 transform (Approach A: decompose)
                *transform = dst_matrix_to_transform(
                    pose.matrix.x_axis.x, pose.matrix.x_axis.y,
                    pose.matrix.y_axis.x, pose.matrix.y_axis.y,
                    pose.offset.x, pose.offset.y,
                    pose.z_index,
                );
            } else {
                // No element at this index for current frame — hide it
                *visibility = Visibility::Hidden;
            }
        }
    }
}
```

### Helper: Get Frame Count from AnimFile

```rust
fn get_frame_count(anim: &AnimFile, bank_name: &str, anim_name: &str) -> usize {
    anim.banks
        .iter()
        .find(|b| b.name == bank_name)
        .and_then(|b| {
            b.animations
                .iter()
                .find(|a| a.name == anim_name)
        })
        .map(|a| a.frames.len())
        .unwrap_or(1)
}
```

---

## 8. Z-Ordering

In Bevy 2D, `Transform.translation.z` determines draw order — higher values render on top. DST's `z_index` is sorted ascending (low z drawn first = background), which maps directly:

```rust
// Map z_index to translation.z with spacing to avoid float precision issues
transform.translation.z = element.z_index * 0.001;
```

The 0.001 spacing keeps all elements within a narrow z-range (so they don't clip with other Bevy entities) while preserving relative ordering.

### Bevy 0.19: ZIndex Component

Bevy 0.19 (upcoming) will introduce dedicated 2D ordering components:

```rust
// Future Bevy 0.19+ API
commands.spawn((
    Sprite { .. },
    ZIndex(element.z_index as i32),  // absolute layer index
));
```

This is cleaner than encoding z-order into `translation.z` and will be the preferred approach once available.

### Third-party: extol_sprite_layer

For explicit layer enums in current Bevy versions:

```rust
#[derive(Component)]
enum SpriteLayer {
    Background,
    Character,
    Foreground,
}

impl LayerIndex for SpriteLayer {
    fn as_z_coordinate(&self) -> f32 {
        match self {
            Self::Background => 0.0,
            Self::Character => 500.0,
            Self::Foreground => 999.0,
        }
    }
}
```

---

## 9. Multi-Build Layering (Skins)

DST supports overlaying multiple builds (e.g., a character base + skin). The current GUI implements this with `collect_build_list()`, which returns builds in reverse order (last build = top layer).

### Bevy Implementation

Each build becomes a separate group of child entities under the same parent:

```rust
#[derive(Component)]
struct DstSkinLayer {
    build_index: usize,     // which build in the skin stack
    enabled: bool,          // visibility toggle
    disabled_symbols: HashSet<String>,  // per-symbol visibility
}

fn spawn_with_skin(
    commands: &mut Commands,
    base_asset: Handle<DstAnimAsset>,
    skin_assets: Vec<Handle<DstAnimAsset>>,
) {
    // Parent entity
    let entity = commands
        .spawn((
            DstAnimator { /* ... */ },
            Transform::default(),
            Visibility::default(),
        ))
        .id();

    // Base layer
    commands.entity(entity).insert(DstSkinLayer {
        build_index: 0,
        enabled: true,
        disabled_symbols: HashSet::new(),
    });

    // Skin layers (each adds more child entities with higher z-offset)
    for (i, skin_handle) in skin_assets.iter().enumerate() {
        // Spawn child entities for skin elements
        // These have higher z-offset than the base layer
        // Skin symbols override base symbols with the same name
        // (by rendering on top with matching z_index)
    }
}
```

### Symbol Override Logic

When multiple builds have the same symbol name, the later build (skin) should visually override the earlier one. In the current codebase, `find_symbol_frame()` searches builds in reverse order — the last match wins.

In Bevy, this happens naturally if skin child entities have the same z_index as base entities but are spawned later (or have a small z bias). Alternatively, explicitly hide base symbol elements when a skin override exists.

---

## 10. Coordinate System Notes

| Issue | DST Convention | Bevy Convention | Handling |
|---|---|---|---|
| **V coordinate flip** | V=0 at bottom | V=0 at top | `ktex.rs` already does `flip_y()`, texture uploaded correctly |
| **Y-axis direction** | Y up | Y up (2D) | Generally consistent; pivot offsets may need Y negation |
| **Matrix storage order** | Row-major `[a,b; c,d]` | Column-major `Mat2::from_cols` | Transpose: `cols = [(a,c), (b,d)]` |
| **Pivot centering** | `dest = elem.tx - sprite_w/2` | `Sprite::anchor = Anchor::Center` | Set anchor to Center, then position at `(tx, ty)` |
| **Alpha blending** | `composite_pixel()` src-over | Standard Bevy alpha blend | Behaves identically |
| **Premultiplied alpha** | KTEX may store premultiplied; `un_premultiply_alpha()` converts | Bevy expects straight alpha | Already handled in `ktex.rs` |

### Pivot Centering Detail

In the existing `atlas.rs`, sprites are centered on their pivot point:

```rust
pivot_x = frame.x - (frame.width / 2.0).floor();
pivot_y = frame.y - (frame.height / 2.0).floor();
dest = (min_x - pivot_x, min_y - pivot_y);
```

In Bevy, this is handled by setting `Sprite::anchor` to `Anchor::Center` and using the element's `(tx, ty)` as the `Transform::translation`. The anchor ensures the sprite is centered at its transform position.

### Matrix Row-Major vs Column-Major

DST stores the matrix as row-major `[a, b, c, d]` representing:

```
| a  b |
| c  d |
```

Bevy's `Mat2` uses column-major storage. The conversion:

```rust
// DST row-major → Bevy column-major
let mat2 = Mat2::from_cols(
    Vec2::new(a, c),  // column 1
    Vec2::new(b, d),  // column 2
);
```

This is equivalent to transposing the matrix.

---

## 11. Plugin Registration

```rust
pub struct DstAnimationPlugin;

impl Plugin for DstAnimationPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<DstAnimAsset>()
            .init_asset_loader::<DstAnimLoader>()
            .add_systems(Update, (
                init_dst_elements,
                update_dst_animation,
            ).chain());
    }
}
```

### App Setup

```rust
fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(DstAnimationPlugin)
        .add_systems(Startup, spawn_dst_character)
        .add_systems(Update, (
            // Camera for 2D rendering
            setup_camera,
        ))
        .run();
}

fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}
```

---

## 12. Usage Example

### Loading and Playing an Animation

```rust
fn spawn_wilson(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    let anim_handle = asset_server.load("anim/wilson.dyn");

    commands.spawn((
        DstAnimator {
            asset: anim_handle,
            bank_name: "wilson".into(),
            anim_name: "idle".into(),
            frame_index: 0,
            timer: Timer::from_seconds(1.0 / 30.0, TimerMode::Repeating),
            playing: true,
        },
        Transform::from_xyz(0.0, 0.0, 0.0),
        Visibility::default(),
    ));
}
```

### Switching Animations at Runtime

```rust
fn switch_animation(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut animators: Query<&mut DstAnimator>,
) {
    for mut animator in &mut animators {
        if keyboard.just_pressed(KeyCode::Space) {
            animator.anim_name = "run".into();
            animator.frame_index = 0;
            animator.timer.reset();
        }
    }
}
```

### Pausing/Resuming

```rust
fn toggle_pause(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut animators: Query<&mut DstAnimator>,
) {
    for mut animator in &mut animators {
        if keyboard.just_pressed(KeyCode::KeyP) {
            animator.playing = !animator.playing;
        }
    }
}
```

### Per-Element Visibility

```rust
fn toggle_element_layer(
    mut animators: Query<&mut DstAnimator>,
) {
    for mut animator in &mut animators {
        // Hide the "arm" layer of the "hand" symbol
        animator.disabled_elements.insert(("hand".into(), "arm".into()));
    }
}
```

---

## Appendix: DST Data Structures Reference

### AnimFile Hierarchy

```
AnimFile
  version: i32
  banks: Vec<AnimBank>
    name: String
    animations: Vec<AnimAnimation>
      name: String
      frame_rate: f32
      frames: Vec<AnimFrame>
        idx: u32
        x, y: f32          (bounding box)
        width, height: f32
        elements: Vec<AnimElement>
          z_index: f32      (draw order, ascending)
          symbol: String    (name reference to BuildSymbol)
          symbol_lower: String (lowercase for case-insensitive lookup)
          frame_num: u32    (which frame of the symbol)
          layer_name: String (visibility grouping)
          a, b, c, d: f32   (2×2 transform matrix)
          tx, ty: f32       (translation)
        events: Vec<String>
```

### BuildFile Hierarchy

```
BuildFile
  version: i32
  name: String
  symbols: Vec<BuildSymbol>
    name: String
    frames: Vec<BuildFrame>
      frame_num: u32
      x, y: f32            (pivot point)
      width, height: f32
      verts: Vec<BuildVert> (6 per quad = 2 triangles)
      image: Option<Arc<RgbaImage>> (set by split_atlas)
    frame_index: HashMap<u32, usize> (O(1) frame lookup)
  atlases: Vec<BuildAtlasRef>
    name: String           (e.g., "atlas-0.tex")
  symbol_index: HashMap<String, usize> (lowercase keys)
```

### BuildVert

```
BuildVert
  x, y, z: f32    (position in sprite space)
  u, v: f32       (UV coordinates, [0,1] range)
  w: u32           (atlas index — which texture atlas)
```

### Key Rendering Constants

| Constant | Value | Description |
|---|---|---|
| V-flip formula | `srcY = (1 - maxV) * height` | DST UV V is flipped |
| Pivot centering | `dest = elem.tx - sprite_w/2` | Sprite centered on pivot |
| Alpha blend | src-over compositing | Standard alpha blending |
| Matrix scale | `scale` arg multiplied into `[a,b,c,d]` | Global scale factor |
| Element sort | z_index ascending, render back-to-front | Lower z drawn first |

### Existing Module Responsibilities

| Module | Role |
|---|---|
| `archive.rs` | .zip/.dyn dispatch, XOR decrypt, tex file merging, OnceCell cache |
| `ktex.rs` | KTEX decode: DXT1/3/5/RGBA/RGB → RGBA, un-premultiply, flip_y |
| `anim.rs` | anim.bin parse: pre-scan hash table, full parse with element sorting |
| `build_file.rs` | build.bin parse: two-pass (skip symbols → read verts → re-read symbols) |
| `atlas.rs` | splitAtlas: UV→pixel crop, V-flip, 6-vert groups, pivot-centered paste |
| `render.rs` | Frame composition: zIndex-sorted overlay, 2×2 transform, alpha blend |
| `gif_export.rs` | GIF encoding with 6-bit quantization |
