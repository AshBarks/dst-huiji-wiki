# DST 动画工具 — Rust 重构技术设计

> 本文档为各模块的详细实现设计，项目计划与阶段划分见 [Plan.md](./Plan.md)
> JS 原文逻辑分析见 [General.md](./General.md)

---

## 1. 项目结构

单 crate `dst-anim-tool`，模块划分如下：

```
src/
  main.rs        — 程序入口，调用 cli
  cli.rs         — clap CLI 参数定义与命令分发
  error.rs       — thiserror 全局错误类型
  reader.rs      — BinaryDataReader（LE 字节序，跳跃读取）
  writer.rs      — BinaryDataWriter
  specs.rs       — 常量、魔数、枚举定义
  hash.rs        — DST 字符串哈希函数
  ktex.rs        — KTEX 头部 / Mipmap / DXT 解码/编码
  anim.rs        — AnimFile 解析（anim.bin / anim.json）
  build.rs       — BuildFile 解析（build.bin / build.json）
  xor.rs         — XOR 流密码（.dyn 解密/加密）
  archive.rs     — ZIP / .dyn 文件分发与处理
  atlas.rs       — splitAtlas：图集裁剪 + 变换 + 拼装精灵帧
  render.rs      — 动画帧合成（多 Element 叠加 + 变换矩阵）
```

---

## 2. 基础库层 → Rust 模块映射

| JS 模块/函数                               | 功能                         | Rust 模块/实现                                                          |
| ------------------------------------------ | ---------------------------- | ----------------------------------------------------------------------- |
| `BinaryDataReader`                         | 带字节序的二进制读取         | `reader::Reader<'a>`                                                    |
| `BinaryDataWriter`                         | 带字节序的二进制写入         | `writer::Writer`                                                        |
| `Ktex / KtexHeader / KtexMipmap`           | KTEX 纹理解码/编码           | `ktex::KtexHeader / KtexMipmap` + `parse()` / `to_image()`             |
| `Specifications`                           | 常量、魔数                   | `specs` 模块：常量 + enum                                              |
| `Platform / PixelFormat / TextureType`     | 枚举                         | `specs::Platform / PixelFormat / TextureType`，派生 `TryFrom<u32>`     |
| `transform()`                              | 仿射变换图像                 | `image::imageops` 或手写仿射变换矩阵                                   |
| `crop()`                                   | 图像裁剪                     | `image::imageops::crop_imm`                                             |
| `resize()`                                 | 图像缩放                     | `image::imageops::resize`                                               |
| `paste()`                                  | 图像粘贴                     | 手写：遍历像素写入目标 `RgbaImage`                                     |
| `newCanvas`                                | 创建画布                     | `image::RgbaImage::new`                                                 |
| `preMultiplyAlpha`                         | 预乘 Alpha                   | 像素迭代：`r *= a/255, g *= a/255, b *= a/255`                         |
| `flipY`                                    | 垂直翻转                     | `image::imageops::flip_vertical`                                        |
| `JSZip`                                    | ZIP 压缩/解压                | `zip` crate                                                             |
| `asyncLoadFile / loadImage / downloadFile` | 文件 I/O                     | `std::fs`（CLI）                                                        |

---

## 3. 二进制读写核心

手写实现，保证对字节序的精细控制。原 JS 使用小端序（LE）。

### Reader

```rust
pub struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self { Self { data, pos: 0 } }
    pub fn pos(&self) -> usize { self.pos }
    pub fn seek(&mut self, pos: usize) { self.pos = pos; }
    pub fn remaining(&self) -> usize { self.data.len().saturating_sub(self.pos) }

    pub fn read_u8(&mut self) -> Result<u8> { ... }
    pub fn read_le_u16(&mut self) -> Result<u16> { ... }
    pub fn read_le_i32(&mut self) -> Result<i32> { ... }
    pub fn read_le_u32(&mut self) -> Result<u32> { ... }
    pub fn read_le_f32(&mut self) -> Result<f32> { ... }
    pub fn read_bytes(&mut self, len: usize) -> Result<&'a [u8]> { ... }
    pub fn read_string(&mut self, len: usize) -> Result<String> { ... }
}
```

关键：`read_le_*` 方法支持**跳跃读取**（传入可选 offset），用于 anim.bin 预扫描和 build.bin 两遍扫描。

### Writer

```rust
pub struct Writer {
    buf: Vec<u8>,
}

impl Writer {
    pub fn write_u8(&mut self, v: u8) { ... }
    pub fn write_le_u16(&mut self, v: u16) { ... }
    pub fn write_le_i32(&mut self, v: i32) { ... }
    pub fn write_le_u32(&mut self, v: u32) { ... }
    pub fn write_le_f32(&mut self, v: f32) { ... }
    pub fn write_bytes(&mut self, data: &[u8]) { ... }
    pub fn write_string(&mut self, s: &str) { ... }
    pub fn into_vec(self) -> Vec<u8> { self.buf }
}
```

---

## 4. 数据结构定义

### Anim 数据结构（`anim.rs`）

```rust
pub struct AnimElement {
    pub z_index: f32,
    pub symbol: String,
    pub frame_num: u32,
    pub layer_name: String,
    pub a: f32, pub b: f32, pub c: f32, pub d: f32,  // 2×2 变换矩阵
    pub tx: f32, pub ty: f32,                          // 平移向量
}

pub struct AnimFrame {
    pub idx: u32,
    pub x: f32, pub y: f32,
    pub width: f32, pub height: f32,
    pub elements: Vec<AnimElement>,
    pub events: Vec<String>,
}

pub struct AnimAnimation {
    pub name: String,
    pub frame_rate: f32,
    pub frames: Vec<AnimFrame>,
}

pub struct AnimBank {
    pub name: String,
    pub animations: Vec<AnimAnimation>,
}

pub struct AnimFile {
    pub version: i32,
    pub banks: Vec<AnimBank>,
}
```

### Build 数据结构（`build.rs`）

```rust
pub struct BuildVert {
    pub x: f32, pub y: f32, pub z: f32,
    pub u: f32, pub v: f32, pub w: f32,
}

pub struct BuildAtlasRef {
    pub name: String,
    pub image: Option<RgbaImage>,  // 解码后赋值
}

pub struct BuildFrame {
    pub frame_num: u32,
    pub duration: u32,
    pub x: f32, pub y: f32,        // Pivot
    pub width: f32, pub height: f32,
    pub verts: Vec<BuildVert>,
    pub image: Option<RgbaImage>,  // splitAtlas 后赋值
}

pub struct BuildSymbol {
    pub name: String,
    pub frames: Vec<BuildFrame>,
}

pub struct BuildFile {
    pub version: i32,
    pub name: String,
    pub symbols: Vec<BuildSymbol>,
    pub atlases: Vec<BuildAtlasRef>,
}
```

---

## 5. KTEX 纹理格式（`ktex.rs`）

### 头部结构

```rust
pub struct KtexHeader {
    pub platform: Platform,
    pub pixel_format: PixelFormat,
    pub texture_type: TextureType,
    pub mipmap_count: u32,
    pub flags: u32,
}

pub struct KtexMipmap {
    pub width: u16,
    pub height: u16,
    pub data_size: u32,
    pub block_data: Vec<u8>,
}

pub struct Ktex {
    pub header: KtexHeader,
    pub mipmaps: Vec<KtexMipmap>,
    pub pre_multiply_alpha: bool,
}
```

### Specification 位域解码

根据 `(specData >> 14) & 0x3FFFF == 0x3FFFF` 判断 PreCave / PostCave，然后按不同位偏移提取字段（见 General.md 六.2）。

### DXT 块解码

手写实现，核心算法：

- **DXT1**：每 4×4 像素块 = 2 个 16bit 颜色 + 4×4 的 2bit 索引。1 bit alpha（当 color0 ≤ color1 时）
- **DXT3**：每块 = 64bit 显式 alpha（4bit/像素）+ DXT1 颜色块
- **DXT5**：每块 = 2 个 8bit alpha 基值 + 6bit 插值索引（3bit/像素）+ DXT1 颜色块
- **RGBA**：直接逐像素读取 R,G,B,A
- **RGB**：逐像素读取 R,G,B，A=255

解码后输出 `image::RgbaImage`。

---

## 6. XOR 流密码（`xor.rs`）

### 密钥与置换表

```rust
const XOR_KEY: [u8; 8] = [141, 142, 143, 144, 145, 146, 147, 148];
const PERMUTATION: [usize; 8] = [5, 3, 6, 7, 4, 2, 0, 1];
```

### 核心逻辑

```rust
pub fn xor_decrypt(data: &[u8]) -> Vec<u8> {
    // 1. 检测是否已解密（前2字节 == "PK"）
    // 2. 对前 16 字节（2 个 8 字节块）做置换 XOR
    // 3. 滑动窗口：后续块用前一块密文作为密钥一部分
    // 4. 返回解密后的 ZIP 数据
}

pub fn xor_encrypt(data: &[u8]) -> Vec<u8> { ... }
```

8 字节块置换 XOR：

```rust
fn xor_cipher_block(block: &[u8], encrypt: bool) -> Vec<u8> {
    if block.len() <= 8 {
        return block.to_vec();
    }
    let mut result = vec![0u8; 8];
    for i in 0..8 {
        let j = PERMUTATION[i];
        result[if encrypt { j } else { i }] = block[if encrypt { i } else { j }] ^ XOR_KEY[i];
    }
    result
}
```

---

## 7. DST 字符串哈希（`hash.rs`）

JS 原文算法（见 General.md 四.3）：

```rust
pub fn dst_hash(s: &str) -> u32 {
    let mut hash: u64 = 0;
    for ch in s.chars() {
        let c = ch.to_ascii_lowercase() as u64;
        hash = (c + (hash << 6) + (hash << 16) - hash) & 0xFFFFFFFF;
    }
    hash as u32
}
```

---

## 8. 图集切割（`atlas.rs`）

将 Build 顶点数据 + Atlas 纹理拼装为每个 Symbol Frame 的独立精灵图。

```rust
pub fn split_atlas(
    build: &mut BuildFile,
    atlas_images: &[RgbaImage],
) -> Result<()> {
    for symbol in &mut build.symbols {
        for frame in &mut symbol.frames {
            let verts = &frame.verts;
            if verts.is_empty() { continue; }

            // 1. 每 6 顶点一组，计算 UV 和 XY 边界
            let (min_u, max_u, min_v, max_v) = calc_uv_bounds(verts);
            let (min_x, max_x, min_y, max_y) = calc_xy_bounds(verts);

            // 2. UV → 像素坐标，从 atlas 裁剪（V 坐标翻转）
            let atlas_idx = verts[0].w as usize;
            let atlas_img = &atlas_images[atlas_idx];
            let src_x = (min_u * atlas_img.width() as f32).round() as u32;
            let src_y = ((1.0 - max_v) * atlas_img.height() as f32).round() as u32;
            let src_w = ((max_u - min_u) * atlas_img.width() as f32).round() as u32;
            let src_h = ((max_v - min_v) * atlas_img.height() as f32).round() as u32;

            let mut sprite = imageops::crop_imm(atlas_img, src_x, src_y, src_w, src_h).to_image();

            // 3. 封闭尺寸匹配：裁剪结果与预期不匹配时缩放
            let expected_w = (max_x - min_x).round() as u32;
            let expected_h = (max_y - min_y).round() as u32;
            if sprite.width() != expected_w || sprite.height() != expected_h {
                sprite = imageops::resize(&sprite, expected_w, expected_h, FilterType::Triangle);
            }

            // 4. 粘贴到以 pivot 为中心的画布
            let pivot_x = frame.x - (frame.width / 2.0).floor();
            let pivot_y = frame.y - (frame.height / 2.0).floor();
            let dest_x = (min_x - pivot_x).round() as i64;
            let dest_y = (min_y - pivot_y).round() as i64;

            let mut canvas = RgbaImage::new(frame.width as u32, frame.height as u32);
            paste(&mut canvas, &sprite, dest_x, dest_y);

            frame.image = Some(canvas);
        }
    }
    Ok(())
}
```

---

## 9. 动画帧合成（`render.rs`）

```rust
pub struct RenderedFrame {
    pub image: RgbaImage,
    pub bounds: BoundingBox,
}

pub struct BoundingBox {
    pub left: f32, pub top: f32,
    pub right: f32, pub bottom: f32,
}

pub fn render_frame(
    anim_frame: &AnimFrame,
    build_list: &[&BuildFile],
    scale: f32,
    offset: (f32, f32),
) -> Result<RenderedFrame> {
    // 1. 计算帧边界框
    let bounds = calc_frame_bounds(anim_frame, build_list, scale, offset);

    // 2. 创建合成画布
    let w = (bounds.right - bounds.left).ceil() as u32;
    let h = (bounds.bottom - bounds.top).ceil() as u32;
    let mut canvas = RgbaImage::new(w, h);

    // 3. 按 zIndex 排序后逐个叠加 Element
    for element in &anim_frame.elements {
        let build_frame = find_symbol_frame(build_list, &element.symbol, element.frame_num);
        if let Some(bf) = build_frame {
            if let Some(ref sprite) = bf.image {
                // 应用变换矩阵 [a,b,c,d,tx,ty] × scale + offset
                let transformed = apply_transform(sprite, element, scale, offset);
                paste(&mut canvas, &transformed, ...);
            }
        }
    }

    Ok(RenderedFrame { image: canvas, bounds })
}
```

---

## 10. 文件入口分发（`archive.rs`）

```rust
pub enum ParsedFile {
    Anim(AnimFile),
    Build(BuildFile),
    Archive(Vec<ParsedFile>),
}

pub fn parse_file(bytes: &[u8], ext: &str) -> Result<ParsedFile> {
    match ext {
        "bin" => parse_bin(bytes),
        "json" => parse_json(bytes),
        "zip" => parse_zip(bytes),
        "dyn" => parse_dyn(bytes),
        _ => Err(Error::UnknownFormat(ext.into())),
    }
}

fn parse_bin(bytes: &[u8]) -> Result<ParsedFile> {
    let mut reader = Reader::new(bytes);
    let magic = reader.read_string(4)?;
    match magic.as_str() {
        "ANIM" => Ok(ParsedFile::Anim(parse_anim(&mut reader)?)),
        "BILD" => Ok(ParsedFile::Build(parse_build(&mut reader)?)),
        _ => Err(Error::InvalidMagic(magic)),
    }
}

fn parse_dyn(bytes: &[u8]) -> Result<ParsedFile> {
    let decrypted = xor_decrypt(bytes);
    parse_zip(&decrypted)
}

fn parse_zip(bytes: &[u8]) -> Result<ParsedFile> {
    // zip crate 解压 → 遍历内部文件 → 递归 parse_file
}
```

---

## 11. 错误类型（`error.rs`）

```rust
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid magic: expected {expected}, got {actual}")]
    InvalidMagic { expected: String, actual: String },

    #[error("read out of bounds: pos {pos}, len {len}")]
    OutOfBounds { pos: usize, len: usize },

    #[error("unknown format: {0}")]
    UnknownFormat(String),

    #[error("unsupported pixel format: {0:?}")]
    UnsupportedPixelFormat(PixelFormat),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("ZIP error: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("image error: {0}")]
    Image(#[from] image::ImageError),
}

pub type Result<T> = std::result::Result<T, Error>;
```

---

## 12. 逆向要点

以下逻辑隐藏在 JS 代码中，重构时必须从原文提取：

| 项目 | 位置 | 说明 |
|------|------|------|
| Specifications 位偏移 | `Navigation-QA1c6I0M.js` | PreCave / PostCave 两种规格的位域定义 |
| XOR 密钥与置换表 | `main-BcHBP8aN.js` 的 `vr()` | 密钥 `[141..156]`，置换表 `[5,3,6,7,4,2,0,1]` |
| DST 哈希常数 | `main-BcHBP8aN.js` 的 `Ne()` | `(hash << 6) + (hash << 16) - hash`，小写化 |
| 方向后缀映射 | `main-BcHBP8aN.js` | 方向标志位 → 后缀字符串的完整映射 |
| DXT 解码算法 | `Navigation-QA1c6I0M.js` 的 `Ktex.toImage()` | DXT1/3/5 块解码的完整实现 |

建议：将每种文件样本的 JS 解析结果（JSON、图像）保存下来，作为 Rust 测试的预期输出（gold standard）。
