# 动画版本结构化 Diff 方案（目标：anim/build 3.3 粒度）

> 本文档供 `/home/hikaru/rust-projects/dst-huiji-wiki` 后续实现时参考。
> 目标不是在本仓库实现，而是把 `dst-anim-tool` 作为库依赖，在 `dst-huiji-wiki` 中完成动画原文件的版本历史与结构化 diff。

## 1. 背景与目标

`dst-anim-tool` 已经可以：

- 解析 `.zip` / `.dyn` / `.bin` 动画包
- 解析 `anim.bin` 为 `AnimFile`
- 解析 `build.bin` 为 `BuildFile`
- 解析 KTEX 纹理、切割精灵、渲染动画帧

本次只做到“3.3 程度”，即：

> 对比两个版本目录中的动画包，输出 **anim.bin / build.bin 解析后的结构化 diff**。

不要求在本阶段实现纹理像素 diff、精灵图 diff、渲染帧视觉 diff；但方案会为后续扩展留好接口。

典型输入：

- 旧版本目录：`/mnt/data/dst-historical/game/client_v_old/data/anim/`
- 当前版本目录：`~/.steam/debian-installation/steamapps/common/Don't Starve Together/data/anim/`

两者都是 `*.zip` 文件集合，zip 内通常包含：

- `anim.bin`
- `build.bin`
- `atlas-*.tex`

也有可能只有其中一部分，例如 `eets_basic.zip` 只有 `anim.bin`，`moon_phases.zip` 只有 `build.bin + atlas-0.tex`。

## 2. 实现范围

建议在 `dst-huiji-wiki` 中新增一个模块，例如：

```text
src/scripts_sync/anim/
├── mod.rs          # 入口、参数、报告
├── snapshot.rs     # 扫描目录/zip，生成版本快照
├── normalize.rs    # AnimFile/BuildFile -> 可序列化规范结构
├── diff.rs         # 结构化 diff 计算
├── report.rs       # 人类可读 diff / JSON 报告
└── history.rs      # 可复用 images-sync 的 CAS + manifest 模式（如需历史管理）
```

核心链路：

```text
版本目录 A ─┐
           ├─ 扫描 .zip -> SHA-256 -> 文件级 diff
版本目录 B ─┘
                │
                ▼
         对发生变化的 zip
                │
                ▼
         解包/读取 anim.bin、build.bin
                │
                ▼
         dst_anim_tool 解析为 AnimFile / BuildFile
                │
                ▼
         规范化为 JSON/文本
                │
                ▼
         结构化 diff（新增/删除/修改到 bank/animation/frame/element/symbol/frame/vert）
                │
                ▼
         输出 unified diff 文本 + 机器可读 JSON
```

## 3. 依赖与使用方式

在 `dst-huiji-wiki/Cargo.toml` 中增加：

```toml
[dependencies]
# 只需要 anim/build 解析时关闭默认 feature，避免编译 GUI/CLI/GIF 工具链
dst-anim-tool = { path = "crates/dst-anim-tool", default-features = false }
```

`dst-anim-tool` 已做 feature 拆分：

- `gui`：eframe / egui / rfd（隐含启用 `cli`，因为 GUI 也使用 rayon）
- `cli`：clap / rayon
- `gif`：gif / which
- 默认：`["gui", "cli", "gif"]`
- 关闭默认 feature 后，核心只依赖 `zip`、`image`、`thiserror`

当前 `dst-anim-tool` 是库 + CLI 的 crate，主要公开 API：

```rust
use dst_anim_tool::archive::{parse_zip, parse_dyn, parse_file_by_path};
use dst_anim_tool::anim::AnimFile;
use dst_anim_tool::build_file::BuildFile;

let data = std::fs::read("some.zip")?;
let parsed = parse_zip(&data)?;

if let Some(anim) = &parsed.anim {
    // 访问 AnimFile
}
if let Some(build) = &parsed.build {
    // 访问 BuildFile
}
```

注意：

- `ParsedArchive.raw_files` 保存了 zip 内原始文件字节，可用于 ZIP 条目级 diff。
- 当前 `AnimFile` / `BuildFile` 结构体是 `pub` 的，但**没有实现 `serde::Serialize`**。
- 因此建议在 `dst-huiji-wiki` 中自定义规范 DTO，而不是直接 serde 序列化 `dst-anim-tool` 的类型。
- 如果后续希望 `dst-anim-tool` 直接输出 canonical JSON，可以再给它增加 `serde`/`normalize` 模块，但本方案不依赖这一点。

## 4. 数据结构映射

### 4.1 AnimFile 规范化字段

`dst-anim-tool::anim::AnimFile` 的结构：

- `version`
- `banks`
  - `name`
  - `animations`
    - `name`
    - `frame_rate`
    - `frames`
      - `idx`
      - `x`, `y`, `width`, `height`
      - `events`
      - `elements`
        - `symbol`
        - `frame_num`
        - `layer_name`
        - `z_index`
        - `a`, `b`, `c`, `d`, `tx`, `ty`

规范 JSON 示例：

```json
{
  "version": 4,
  "banks": [
    {
      "name": "wilson",
      "animations": [
        {
          "name": "idle",
          "frame_rate": 15.0,
          "frames": [
            {
              "idx": 0,
              "x": 0.0,
              "y": 0.0,
              "width": 100.0,
              "height": 100.0,
              "events": ["sound"],
              "elements": [
                {
                  "symbol": "body",
                  "frame_num": 1,
                  "layer_name": "body",
                  "z_index": 0.0,
                  "a": 1.0,
                  "b": 0.0,
                  "c": 0.0,
                  "d": 1.0,
                  "tx": 0.0,
                  "ty": 0.0
                }
              ]
            }
          ]
        }
      ]
    }
  ]
}
```

注意忽略：

- `AnimElement.symbol_lower`：它是由 `symbol` 派生的小写索引，不需要进入 diff。
- `AnimFile` 内部没有其它派生字段。

### 4.2 BuildFile 规范化字段

`dst-anim-tool::build_file::BuildFile` 的结构：

- `version`
- `name`
- `atlases`
  - `name`
- `symbols`
  - `name`
  - `frames`
    - `frame_num`
    - `duration`
    - `x`, `y`, `width`, `height`
    - `verts`
      - `x`, `y`, `z`, `u`, `v`, `w`

规范 JSON 示例：

```json
{
  "version": 6,
  "name": "moon_phases",
  "atlases": [
    { "name": "atlas-0.tex" }
  ],
  "symbols": [
    {
      "name": "moon_full",
      "frames": [
        {
          "frame_num": 0,
          "duration": 1,
          "x": 0.0,
          "y": 0.0,
          "width": 128.0,
          "height": 128.0,
          "verts": [
            { "x": 0.0, "y": 0.0, "z": 0.0, "u": 0.0, "v": 0.0, "w": 0 }
          ]
        }
      ]
    }
  ]
}
```

注意忽略 Build 中与“解析后/切割后”相关的派生字段：

- `BuildFrame.image`
- `BuildFrame.dest_x`
- `BuildFrame.dest_y`
- `BuildFrame.canvas_w`
- `BuildFrame.canvas_h`
- `BuildFrame.spans`
- `BuildFile.symbol_index`

这些字段只有在 `split_atlas` 之后才有意义，不属于源文件本身的语义。

## 5. 规范化与对齐规则

直接对二进制或原始 JSON 做 diff 会产生大量噪音。必须做规范化。

### 5.1 列表排序

为了消除“二进制里顺序变化但语义未变”的噪音，建议在规范 JSON 中排序：

- `AnimFile.banks`：按 `name` 排序
- `AnimBank.animations`：按 `name` 排序
- `AnimAnimation.frames`：按 `idx` 排序
- `AnimFrame.elements`：按 `(z_index, symbol, layer_name, frame_num)` 排序
- `BuildFile.symbols`：按 `name` 排序
- `BuildSymbol.frames`：按 `frame_num` 排序
- `BuildFrame.verts`：**保持源文件顺序**，因为 vertex 顺序参与三角形带语义，不能简单排序

### 5.2 浮点归一化

`anim.bin` / `build.bin` 中有很多 `f32`。建议：

- 在比较时使用 epsilon，例如 `1e-4` 或 `1e-6`
- 在输出 canonical JSON 时四舍五入到固定精度，例如 6 位小数
- 避免直接输出完整 `f32` 原始位模式，否则同一数值可能因解析/存储细节产生 false positive

### 5.3 忽略索引型/派生字段

- `symbol_lower` 不输出
- `symbol_index` 不输出
- `image` / `spans` / `dest_*` / `canvas_*` 不输出

### 5.4 对元素做稳定匹配

对于 `AnimFrame.elements`，新旧版本中元素的排列可能不同。直接按数组下标 diff 会误报“删除+新增”。

建议匹配键：

```text
symbol + layer_name + frame_num
```

如果这三个字段都相同，就认为这是同一个 element，再比较：

- `z_index`
- `a`, `b`, `c`, `d`
- `tx`, `ty`

如果遇到同一个 `symbol + layer_name + frame_num` 出现多次（少见），可以退化为按元素在帧内的出现顺序匹配，或把重复项也纳入 diff。

### 5.5 对 Build 顶点做合理比较

`BuildFrame.verts` 建议保留顺序比较。对于大多数情况，顶点变化意味着几何/UV 变化。

如果后续发现某些工具导出时顶点顺序频繁变化但视觉不变，可以再增加“几何归一化”或“渲染结果 diff”来辅助判断；但 3.3 阶段先按原始顺序展示。

## 6. Diff 类型与输出格式

### 6.1 文件级 diff（前置筛选）

先比较两个目录的文件名 + SHA-256：

- added
- removed
- modified
- unchanged

只有 added / removed / modified 的 zip 才进入后续解析。

### 6.2 ZIP 条目级 diff（辅助定位）

对于 modified 的 zip，比较内部条目：

- `anim.bin` 是否存在、大小、CRC
- `build.bin` 是否存在、大小、CRC
- `atlas-*.tex` 是否存在、大小、CRC

这样可以快速知道：

- 只改了 `anim.bin`：动画逻辑变化
- 只改了 `build.bin`：顶点/UV/帧定义变化
- 只改了 `atlas-*.tex`：纹理变化（本阶段不深入）
- 多个同时变化

### 6.3 结构化 diff（核心）

对 `AnimFile` / `BuildFile` 规范化后，分别输出结构化变更。

建议机器可读 JSON 结构：

```json
{
  "zip": "abigail_vial_fx.zip",
  "anim": {
    "changed": true,
    "summary": {
      "added_banks": [],
      "removed_banks": [],
      "modified_banks": 1,
      "added_animations": 0,
      "removed_animations": 0,
      "modified_animations": 1,
      "changed_frames": 3,
      "changed_elements": 5
    },
    "details": [
      {
        "bank": "abigail_vial_fx",
        "animation": "idle",
        "type": "modified",
        "changes": [
          {
            "frame_idx": 0,
            "kind": "element_transform",
            "element": "vial/liquid",
            "field": "ty",
            "old": 10.0,
            "new": 12.5
          }
        ]
      }
    ]
  },
  "build": {
    "changed": false,
    "summary": null,
    "details": []
  }
}
```

人类可读展示可以使用 `dst-huiji-wiki` 已有的 `similar` crate：

```rust
let old_text = canonical_json_string(old);
let new_text = canonical_json_string(new);
let diff = similar::TextDiff::from_lines(&old_text, &new_text);
println!("{}", diff.unified_diff());
```

这样能直接展示到终端或 WebUI。

### 6.4 推荐的 Diff 分类

对 anim/build 的变更类型可以分为：

- `added` / `removed`：整个 bank / animation / symbol / frame / element 新增或删除
- `modified`：同名实体存在，但内部字段变化
- 字段级变更：
  - `frame_rate`
  - `frame_count`
  - `frame_size`（x/y/width/height）
  - `events`
  - `element_transform`（a/b/c/d/tx/ty）
  - `z_index`
  - `element_symbol`
  - `element_frame_num`
  - `symbol_frame`
  - `symbol_duration`
  - `symbol_pivot`
  - `symbol_size`
  - `vert`
  - `atlas`

## 7. 版本历史管理建议

虽然本次重点是 diff，但为了方便多个版本连续对比，建议沿用 `dst-huiji-wiki` 中 `images-sync` 已有的 CAS + manifest 模式：

```text
history/
  objects/<hash[:2]>/<sha256>       # 原始 zip 或解析 JSON，按内容寻址
  manifests/<build>.json            # 每个版本的清单 + parent diff
```

manifest 可以记录：

```json
{
  "build": "client_v_old",
  "parent_build": null,
  "complete": true,
  "files": {
    "abigail_vial_fx.zip": {
      "sha256": "...",
      "size": 231250
    }
  },
  "anim_meta": {
    "abigail_vial_fx.zip": {
      "anim_sha256": "...",
      "build_sha256": "..."
    }
  }
}
```

如果不想做完整历史存储，也可以只提供“两个目录直接对比”的 CLI，不落历史。两种方式可以共存。

## 8. 实现步骤建议

1. **接入依赖**：在 `dst-huiji-wiki` 中添加 `dst-anim-tool` path 依赖，先写一个最简单的 `parse <zip>` 验证 API。
2. **扫描两个目录**：计算文件 SHA-256，得到 added/removed/modified/unchanged。
3. **对 modified 或指定 zip 做 ZIP 条目级比较**：列出 anim/build/tex 的变化。
4. **实现 normalize**：把 `AnimFile` / `BuildFile` 转成规范 JSON/文本。
5. **实现结构化 diff**：先做“整段 canonical JSON 的文本 diff”，再做“按 bank/animation/element 的结构化 diff”。
6. **输出报告**：
   - 终端 unified diff
   - `--report-json` 机器可读 JSON
7. **接入 WebUI（可选）**：复用现有 diff 页面/API 风格，提供动画 diff 的 Web 展示。
8. **历史存储（可选）**：复用 `images-sync` 的 `ObjectStore` / `ManifestStore`。

## 9. 风险与注意事项

- **当前 `dst-anim-tool` 没有 serde 支持**：不要在别的项目里直接 `serde_json::to_string(&AnimFile)`，需要自己定义 DTO。
- **浮点噪声**：必须做精度归一化或 epsilon 比较。
- **顺序噪声**：banks/animations/symbols/frames 要排序；elements 要按稳定键匹配。
- **派生字段**：`image`、`spans`、`symbol_index` 等不要进入 diff。
- **zip 内可能缺少 anim.bin 或 build.bin**：diff 逻辑要允许 `Option`，一个文件可能只有其中一种。
- **`.dyn` 文件**：如果未来要支持动态皮肤包，需要先 `xor_decrypt` 再解析；本阶段以 `.zip` 为主。
- **数据量**：两个目录分别约 1.5~1.6 GB，但实际版本间增量很小。全量扫描/哈希第一次较慢，之后增量很快。
- **不要把所有原始 zip 放入普通 Git 历史**：建议原始 zip 进对象存储，manifest/JSON 进 Git 或普通文件。

## 10. 后续扩展方向

- 3.4：KTEX 解码后做纹理/精灵像素 diff
- 3.5：用 `dst-anim-tool::render` 渲染新旧动画帧做视觉 diff
- 在 `dst-anim-tool` 中增加可选 `serde` / `normalize` 模块，让库直接输出 canonical JSON
- （已完成）`dst-anim-tool` 的 GUI/CLI/GIF 依赖已拆分为 optional feature；作为纯解析库时使用 `default-features = false` 即可

---

以上是实现思路和大致方案，后续可在 `dst-huiji-wiki` 中按此文档落地。
