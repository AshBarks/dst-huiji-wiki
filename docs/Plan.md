# DST 动画工具 - 项目计划

> 将 [dont-starve-anim-tool](https://dont-starve-anim-tool.pages.dev/) 的 JS 实现重构为 Rust CLI + GUI 工具

---

## 一、项目定位

- **当前状态**：所有计划功能已实现 — CLI（extract/split/render/list/info/decrypt/decode/preview）+ GUI 预览（GIF/PNG 导出）
- **后续扩展**：KTEX 编码、build/anim 写入、crate 拆分

---

## 二、技术选型

| 模块 | 选型 | 理由 |
|------|------|------|
| 项目结构 | 单 crate `dst-anim-tool` | 初期快速迭代，后期稳定后可拆分 |
| 二进制读写 | 手写 `Reader` / `Writer` | DST 格式有预扫描跳跃读取，完全可控 |
| DXT 解码 | 手写实现 | DXT1/3/5→RGBA 算法不复杂，无成熟 Rust crate |
| 图像处理 | `image`（仅 png feature） | PNG 读写、裁剪/缩放/翻转；禁用默认 features 减少编译时间和二进制体积 |
| ZIP 处理 | `zip` | 纯 Rust，API 简单，流式解压 |
| XOR 解密 | 手写 | 8 字节块置换，逻辑简单 |
| 错误处理 | `thiserror` | 精确错误枚举，便于诊断 |
| CLI | `clap` (derive) | 主流框架，8 个子命令 |
| GUI | `eframe` + `egui` | 即时模式 GUI，跨平台，适合预览工具 |
| GIF 导出 | `gif` + 可选 `ffmpeg` | 内置 6-bit 量化编码；ffmpeg 可选提供更高质量 |
| 并行 | `rayon` | GIF/PNG 导出帧并行渲染 |
| 文件对话框 | `rfd` | 原生文件打开/保存对话框 |
| 哈希函数 | 手写 `dst_hash` | 类似 djb2，JS 原文有明确算法 |

---

## 三、模块划分

```
src/
  main.rs        — 程序入口，调用 cli::run()
  cli.rs         — clap CLI：extract/split/render/list/info/decrypt/decode/preview
  error.rs       — thiserror 错误类型
  reader.rs      — Binary Reader（LE 字节序，跳跃读取）
  writer.rs      — Binary Writer（LE 字节序）
  specs.rs       — 常量、魔数、枚举、KtexSpec 位域、方向后缀映射
  hash.rs        — DST 字符串哈希函数
  ktex.rs        — KTEX 头部 / Mipmap / DXT1/3/5/RGBA/RGB 解码
  anim.rs        — AnimFile 解析与写入（anim.bin）
  build_file.rs  — BuildFile 解析与写入（build.bin）
  xor.rs         — XOR 流密码（.dyn 解密/加密）
  archive.rs     — .zip/.dyn 文件分发与处理（Arc<Vec<u8>> 共享纹理数据）
  atlas.rs       — splitAtlas：图集裁剪 + 变换 + 拼装精灵帧
  render.rs      — 动画帧合成（ElementData + PreparedFrame 预计算）
  gif_export.rs  — GIF 编码（6-bit 量化 + ffmpeg 可选）
  ui.rs          — egui GUI 预览（多档案、树导航、后台渲染、GIF/PNG 导出）
```

---

## 四、依赖清单

```toml
[dependencies]
clap = { version = "4", features = ["derive"] }
eframe = "0.30"
egui = "0.30"
gif = "0.13"
image = { version = "0.25", default-features = false, features = ["png"] }
rfd = "0.15"
rayon = "1.10"
thiserror = "2"
which = "7"
zip = "2"
```

---

## 五、开发阶段

### 阶段 1 — 基础设施 ✅

**目标**：二进制读写、错误类型、常量枚举、哈希函数

| 文件 | 内容 |
|------|------|
| `error.rs` | 全局错误枚举：`InvalidMagic`、`OutOfBounds`、`UnknownFormat`、`UnsupportedPixelFormat`、`Io`、`Zip`、`Gif`、`Ui` |
| `reader.rs` | `Reader<'a>`：`read_u8/le_u16/le_i32/le_u32/le_f32/read_bytes/read_string/seek/pos` + `_at()` 跳跃读取 |
| `writer.rs` | `Writer`：对应写入方法 + `into_vec()` |
| `specs.rs` | 魔数 `ANIM/BILD/KTEX`、枚举 `Platform/PixelFormat/TextureType/Direction`、方向后缀映射、`KtexSpec` 位域、`detect_spec()` |
| `hash.rs` | `dst_hash(s: &str) -> u32`：字节级迭代 + `to_ascii_lowercase()` |

### 阶段 2 — anim & build 二进制解析 ✅

| 文件 | 内容 |
|------|------|
| `anim.rs` | `AnimFile/Bank/Animation/Frame/Element` 数据结构 + `parse_anim()` + `write_anim()` |
| `build_file.rs` | `BuildFile/Symbol/Frame/Vert/AtlasRef` 数据结构 + `parse_build()` + `write_build()` |

**关键实现点**：
- anim.bin 预扫描阶段：跳跃读取偏移定位字符串哈希表
- build.bin 两遍扫描：先跳过 Symbol 定位顶点数组，再回头正式解析
- 字符串哈希表还原

### 阶段 3 — ZIP / .dyn 处理 ✅

| 文件 | 内容 |
|------|------|
| `xor.rs` | XOR 流密码：8 字节块置换 `[5,3,6,7,4,2,0,1]`，顺序块处理 |
| `archive.rs` | 文件分发：按扩展名路由 → 解压 → 按内部文件名分发；`Arc<Vec<u8>>` 共享纹理数据 |

**关键实现点**：
- .dyn 检测：前 2 字节非 "PK" 则先 XOR 解密
- ZIP 内部文件遍历：识别 .bin / .tex
- `tex_files()` 返回 `HashMap<String, Arc<Vec<u8>>>`，clone 仅增加引用计数

### 阶段 4 — KTEX 纹理解码 ✅

| 文件 | 内容 |
|------|------|
| `ktex.rs` | `KtexHeader / KtexMipmap` 结构 + Specification 位域解码 + DXT1/3/5/RGBA/RGB → `image::RgbaImage` |

**关键实现点**：
- PreCave / PostCave 规格位域偏移差异
- 完整块批量行拷贝（16 字节/行），边缘块逐像素回退
- `un_premultiply_alpha`：整数 `div_ceil()` 替代浮点除法
- `flip_y`：行级 `copy_from_slice` + `copy_within` 替代逐字节 swap

### 阶段 5 — Atlas 切割 ✅

| 文件 | 内容 |
|------|------|
| `atlas.rs` | `split_atlas(atlas_img, frames)`：UV→像素坐标裁剪 + 封闭尺寸匹配 + pivot 定位粘贴 |

**关键实现点**：
- 每 6 顶点一组计算 UV/XY 边界
- V 坐标翻转：`srcY = (1 - maxV) * height`
- 裁剪尺寸与预期不匹配时缩放
- 粘贴到以 pivot 为中心的画布

### 阶段 6 — 动画帧合成 ✅

| 文件 | 内容 |
|------|------|
| `render.rs` | 帧合成：`compute_frame_elements()` + `compute_bounds_from_elements()` + `render_frame_with_elements()`；`prepare_animation_frames()` 批量预计算 |

**关键实现点**：
- `ElementData` 预计算：`find_symbol_frame` 查找 + sprite 引用 + 变换参数一次完成
- `PreparedFrame`：每帧的 elements + bounds 缓存，避免渲染时重复查找
- `render_frame_with_elements()`：直接使用预计算数据，跳过 symbol 查找
- `composite_pixel`：`src_a == 0` / `src_a == 255` / `dst_a == 0` 快速路径
- 仿射变换：恒等变换快速路径 + 均匀缩放路径 + 通用路径
- 通用路径按行做 span 预拒绝：逆映射该行两端源 y 坐标，整行对应源行全部透明时跳过（避免大旋转精灵的无效扫描）

### 阶段 7 — CLI 串联 ✅

| 文件 | 内容 |
|------|------|
| `cli.rs` | clap 8 子命令：`extract`、`split`、`render`、`list`、`info`、`decrypt`、`decode`、`preview` |
| `main.rs` | 入口串联 |

**CLI 命令**：

```
dst-anim-tool extract -i <input>... <output-dir>
dst-anim-tool split   -i <input>... [--skin <skin>] <output-dir>
dst-anim-tool render  -i <input>... [--skin <skin>] <anim-path> <output-dir>
dst-anim-tool list    -i <input>...
dst-anim-tool info    -i <input>...
dst-anim-tool decrypt <input.dyn> <output.zip>
dst-anim-tool decode  <input> <output-dir>
dst-anim-tool preview [-i <input>...]
```

### 阶段 8 — GUI 预览 + GIF 导出 ✅

| 文件 | 内容 |
|------|------|
| `ui.rs` | egui GUI：多档案拖放、动画/银行/帧树导航、后台帧渲染、GIF/PNG 导出（rayon 并行） |
| `gif_export.rs` | GIF 编码：6-bit 颜色量化（`Vec<u16>` 直接索引表 64³）、可选 ffmpeg PNG→GIF |

**关键实现点**：
- `PreparedFrame` move 到线程，避免 clone 整个 `BuildFile` + `AnimFile`
- 后台渲染线程 + mpsc 通道异步传回帧图像（只渲染缺失帧，避免重复渲染）
- 帧缓存 `HashMap<usize, FrameCacheEntry>` 含 CPU image + GPU texture
- **缓存预算自适应动画规模**：按每帧 snapped 边界精确求和，`clamp(总占用, 512MB, 1536MB)`，保证整段动画可驻留缓存
- egui 纹理**延迟创建**：后台只缓存图像，纹理在帧首次显示时创建，避免一次性大规模 GPU 上传阻塞 UI
- 帧图像按需替换（字节账目正确、LRU 队列去重），重复播放 loop 2+ 全部命中缓存，无主线程重渲染

---

## 六、测试策略

| 层级 | 方式 | 说明 |
|------|------|------|
| 单元测试 | `#[test]` 在各模块（115 个） | 读写器、哈希、DXT 解码、枚举转换、XOR 往返、GIF 量化、帧缓存账目 |
| 集成测试 | 模块内 `#[test]` 用真实文件 | 用 `data/` 中的 .zip/.dyn 端到端验证 |
| 样本数据 | `data/` 目录（gitignored） | 符号链接到 DST 游戏文件 |

---

## 七、后续路线

| 阶段 | 内容 | 状态 |
|------|------|------|
| V2 | UI 框架提供动画序列预览和交互 | ✅ 已完成（egui） |
| V3 | KTEX 编码（RGBA → DXT → .tex 写入） | 待开发 |
| V3 | build.bin / anim.bin 写入（修改后的数据重新打包） | 基础实现已有，需集成测试 |
| V4 | crate 拆分（core / io / cli）便于作为 library 被其他项目引用 | 待开发 |
