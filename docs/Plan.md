# DST 动画工具 - 项目计划

> 将 [dont-starve-anim-tool](https://dont-starve-anim-tool.pages.dev/) 的 JS 实现重构为 Rust CLI 工具

---

## 一、项目定位

- **当前目标**：CLI 工具，支持解压 .dyn/.zip、解析 anim/build、切割精灵图、导出 PNG 帧序列
- **后续扩展**：添加 UI 框架提供动画序列预览

---

## 二、技术选型（已确定）

| 模块 | 选型 | 理由 |
|------|------|------|
| 项目结构 | 单 crate `dst-anim-tool` | 初期快速迭代，后期稳定后可拆分 |
| 二进制读写 | 手写 `Reader` / `Writer` | DST 格式有预扫描跳跃读取，完全可控 |
| DXT 解码 | 手写实现 | DXT1/3/5→RGBA 算法不复杂，无成熟 Rust crate |
| 图像处理 | `image` | PNG 读写、裁剪/缩放/翻转，生态成熟 |
| ZIP 处理 | `zip` | 纯 Rust，API 简单，流式解压 |
| XOR 解密 | 手写 | 8 字节块置换 + 滑动窗口，逻辑简单 |
| 错误处理 | `thiserror` | 精确错误枚举，便于诊断 |
| CLI | `clap` (derive) | 主流框架，后续加子命令方便 |
| JSON | `serde` + `serde_json` | anim.json / build.json 解析 |
| 哈希函数 | 手写 `dst_hash` | 类似 djb2，JS 原文有明确算法 |

---

## 三、模块划分

```
src/
  main.rs        — 程序入口，调用 cli
  cli.rs         — clap CLI 参数定义与命令分发
  error.rs       — thiserror 全局错误类型定义
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

## 四、依赖清单

```toml
[dependencies]
image = "0.25"       # 图像读写与 imageops
zip = "2"            # ZIP 解压
thiserror = "2"      # 错误类型定义
clap = { version = "4", features = ["derive"] }  # CLI
serde = { version = "1", features = ["derive"] }  # 序列化
serde_json = "1"     # JSON 解析
log = "0.4"          # 日志 facade
env_logger = "0.11"  # 日志后端（CLI）
```

---

## 五、开发阶段

### 阶段 1 — 基础设施

**目标**：二进制读写、错误类型、常量枚举、哈希函数

| 文件 | 内容 |
|------|------|
| `error.rs` | 全局错误枚举：`InvalidMagic`、`OutOfBounds`、`FormatMismatch`、`IoError` 等 |
| `reader.rs` | `Reader<'a>`：`read_u8/le_u16/le_i32/le_u32/le_f32/read_bytes/read_string/seek/pos` |
| `writer.rs` | `Writer`：对应写入方法 + `into_vec()` |
| `specs.rs` | 魔数 `ANIM/BILD/KTEX`、枚举 `Platform/PixelFormat/TextureType/Direction`、方向后缀映射 |
| `hash.rs` | `dst_hash(s: &str) -> u32` |

**验证**：单元测试各读写方法、哈希值对比 JS 原文

### 阶段 2 — anim & build 二进制解析

**目标**：正确解析 anim.bin 和 build.bin 为 Rust 数据结构

| 文件 | 内容 |
|------|------|
| `anim.rs` | `AnimFile/Bank/Animation/Frame/Element` 数据结构 + `parse_anim(reader)` |
| `build.rs` | `BuildFile/Symbol/Frame/Vert/AtlasRef` 数据结构 + `parse_build(reader)` |

**关键实现点**：
- anim.bin 预扫描阶段：跳跃读取偏移定位字符串哈希表
- build.bin 两遍扫描：先跳过 Symbol 定位顶点数组，再回头正式解析
- 字符串哈希表还原

**验证**：用真实 .bin 文件测试，对比 JS 原文的解析结果

### 阶段 3 — ZIP / .dyn 处理

**目标**：解压 .zip 和 .dyn（XOR 解密后解压）

| 文件 | 内容 |
|------|------|
| `xor.rs` | XOR 流密码：密钥生成、置换表、滑动窗口解密 |
| `archive.rs` | 文件分发：按扩展名路由 → 解压 → 按内部文件名分发 |

**关键实现点**：
- .dyn 检测：前 2 字节非 "PK" 则先 XOR 解密
- ZIP 内部文件遍历：识别 .bin / .json / .tex / .png
- 递归分发到各解析器

**验证**：用真实 .zip / .dyn 文件测试，确认解压后文件完整

### 阶段 4 — KTEX 纹理解码

**目标**：将 KTEX DXT 压缩数据解码为 RGBA 图像

| 文件 | 内容 |
|------|------|
| `ktex.rs` | `KtexHeader / KtexMipmap` 结构 + Specification 位域解码 + DXT1/3/5/RGBA/RGB → `image::RgbaImage` |

**关键实现点**：
- PreCave / PostCave 规格位域偏移差异
- DXT1：4 字节/块，1 bit alpha
- DXT3：8 字节显式 alpha + 4 字节颜色
- DXT5：8 字节插值 alpha + 4 字节颜色
- RGBA：直接像素数据
- preMultiplyAlpha 标志处理

**验证**：用真实 .tex 文件解码，输出 PNG 与 JS 原文结果对比

### 阶段 5 — Atlas 切割

**目标**：splitAtlas — 将 Build 顶点数据 + Atlas 纹理拼装为精灵帧

| 文件 | 内容 |
|------|------|
| `atlas.rs` | `split_atlas(atlas_img, frames) -> Vec<RgbaImage>`：UV→像素坐标裁剪 + 封闭尺寸匹配 + pivot 定位粘贴 |

**关键实现点**：
- 每 6 顶点一组计算 UV/XY 边界
- V 坐标翻转：`srcY = (1 - maxV) * height`
- 裁剪尺寸与预期不匹配时缩放
- 粘贴到以 pivot 为中心的画布

**验证**：用真实 build + atlas 数据，对比 JS 原文的精灵帧输出

### 阶段 6 — 动画帧合成

**目标**：将 Anim Element 引用 + Build Frame 精灵图叠加渲染

| 文件 | 内容 |
|------|------|
| `render.rs` | 帧合成：按 zIndex 排序，对每个 Element 应用变换矩阵叠加精灵图，输出边界框 + 合成图像 |

**关键实现点**：
- Element 变换矩阵 [a,b,c,d,tx,ty] × BuildFrame pivot
- Symbol Frame 查找：resolve alias + 遍历 buildList
- 帧边界计算

**验证**：用真实 anim + build 数据渲染帧，对比 JS 原文

### 阶段 7 — CLI 串联

**目标**：完整 CLI 工具，端到端流程可用

| 文件 | 内容 |
|------|------|
| `cli.rs` | clap 子命令：`extract`（解压）、`split`（切割精灵帧）、`render`（渲染动画帧） |
| `main.rs` | 入口串联 |

**CLI 命令设计**：

```
dst-anim-tool extract <input.zip|.dyn> <output-dir>
dst-anim-tool split   <input.zip|.dyn> <output-dir>
dst-anim-tool render  <input.zip|.dyn> <anim-name> <output-dir>
```

**验证**：端到端测试，从输入文件到输出 PNG

---

## 六、测试策略

| 层级 | 方式 | 说明 |
|------|------|------|
| 单元测试 | `#[test]` 在各模块 | 读写器、哈希、DXT 解码、枚举转换 |
| 集成测试 | `tests/` 目录 | 用真实 .bin / .tex / .zip 文件端到端验证 |
| 对比验证 | JS 原文输出作为 gold standard | Rust 输出与 JS 工具输出逐像素比对 |
| 样本数据 | `tests/data/` 目录 | 收集各类格式的测试文件 |

---

## 七、后续路线

| 阶段 | 内容 |
|------|------|
| V2 | 添加 UI 框架（如 egui / Tauri）提供动画序列预览和交互 |
| V3 | KTEX 编码（RGBA → DXT → .tex 写入） |
| V3 | build.bin / anim.bin 写入（修改后的数据重新打包） |
| V4 | crate 拆分（core / io / cli）便于作为 library 被其他项目引用 |