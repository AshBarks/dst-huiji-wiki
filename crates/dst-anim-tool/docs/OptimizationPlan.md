# 下游反馈核实与优化方案

> 对象：`docs/Feedback-dst-auto-chess-btoy.md`
> 核实基准：本仓 HEAD `36cf8ff`（与反馈标注一致），2026-10-01，Linux
> 核实方式：源码走查 + 实际构建 `target/release/dst-anim-tool` 复现 + 两个临时集成测试（bound 误差复现、全量 fps 扫描；核实后已删除，工作区无残留）
>
> **移植说明（2026-10-01，宿主 `dst-huiji-wiki` vendored crate）**：本方案已整体吸收进
> `crates/dst-anim-tool`，并做了以下适配：
> - edition 2021（workspace 统一），新增代码中的 let-chain 改写为嵌套 if；
> - KTEX 统一走共享 crate `dst-ktex`（`parse`/`decode_rgba`），因此 `info --json`
>   的 tex 字段名为 `compression`（原方案的 `KtexHeader::pixel_format` 已不存在）；
> - 原有 `chunks_exact` 用法统一为 `as_chunks`，保持 `clippy -D warnings` 全绿；
> - 数据依赖测试沿用 `#[ignore = "requires local DST game data (data/anim symlink)"]`，
>   `tests/cli.rs` 同时声明 `required-features = ["cli"]`；宿主本地全量为
>   110 lib + 6 bin + 6 integration（CI 模式 72 passed / 50 ignored）。

---

## 0. 核实结论

| 编号 | 原文结论 | 核实 | 证据摘要 |
| --- | --- | --- | --- |
| P0-1 | `bounds` 是贴图矩形并集，非墨迹包围盒 | **属实** | `render.rs:297-340` 用 `sprite.width()/height()` 四角变换，完全不看 alpha / `spans`；实测 tallbird `idle_side` 40 帧：rect 比墨迹 bottom 低 **24.34px**（原文 24.3），top 高 5.08px（原文 5.1），极差 0.08px |
| P0-2 | `list` / `info` 空结果退出码 0 | **属实（`info` 需分场景）** | `list -i pig_build.zip` → stderr + `exit=0`；`info` 对 build-only 包仍打印 build 信息，退出 0 是正确的（下游 3566 次 `info` 普查依赖此行为），仅对「完全无数据」才应报错 |
| P0-3 | 找不到 symbol 时静默丢元素 | **属实** | `render.rs:267-289` `find_symbol_frame` 返回 `None` 后直接跳过，无计数/告警/退出码 |
| P1-4 | 库/CLI 无「按层名隐藏」接口 | **属实** | `disabled_elements: HashSet<(symbol_lower, layer_name)>`（`render.rs:230-261`）；`layer_name` 解析时保留原大小写（`anim.rs:178-194`），匹配不归一化；无 `disabled_layers` |
| P1-5 | fork 的 5 项能力官方 CLI 全无 | **属实** | `cli.rs:23-34` Render 仅 `input/skin/symbol_map/build/anim_path/output_dir` |
| P1-6 | 渲染不输出时序与逐帧元数据 | **属实，且风险已实际存在** | `cmd_render` 丢弃 `frame_rate`，只写 `frame_{i:03}.png`；扫描 2800 个含 anim.bin 的包：**21 个非 30fps**（14×12、5×24、1×25、1×27），硬编码 30fps 必然静默变速 |
| P1-7 | 无批处理入口，重复解码同一 build | **属实** | 每次进程都完整 `load_archives + split_atlas`（`cli.rs:300-347`） |
| P1-8 | `list`/`info` 无机器可读输出、无来源归属 | **属实** | 文本输出靠正则；`ParsedArchive::merge` 对 anim/build 后写覆盖（`archive.rs:40-54`），实测 `info -i ds_tallbird_basic.zip pig_build.zip` 只显示 pig_build，无来源标注 |
| P2-9 | `render.rs` 无条件 use rayon，纯库消费者必须开 `cli` | **部分不属实（证据过时）** | `36cf8ff` 中 `render.rs:3-4` 已是 `#[cfg(any(feature="cli", feature="gui"))]`；`cargo check --no-default-features` 通过。残留问题：`_par` 并行渲染与 rayon 仍被绑在 `cli` 上，会顺带拖入 clap/tracing-subscriber |
| P2-10 | wasm 目标无保障 | **属实（文档缺口）** | `load_archives` 用 `std::fs`、render 并行依赖 rayon；README 无 wasm 边界声明、无 CI 检查 |
| P2-11 | fork 漂移与契约测试缺失 | **属实（以下游侧为主）** | 上游可通过落地 P1-4/5 + 打 tag + 金样测试来收敛 |
| P2-12 | CLI 设计细节 | **基本属实，1 条描述有误** | `--build` 变长参数确实必须 `--` 分隔（已复现 clap 报错）；帧命名契约未文档化；`disabled_elements` 不归一化；`info` 合并静默覆盖；`compute_bounds` 的 `cw/ch` **并非未使用**（参与 center 计算），只是与 `sw/sh` 重复定义；GUI/GIF 与 CLI 能力不对齐属实 |

**总评**：P0 三条、P1 五条、P2 四条全部成立（P2-9 的代码证据不准确，但结论方向可保留）；反馈给出的复现数据与本次独立复现一致，可信度高。

---

## 1. 优化方案

设计原则：

- 先修「静默错误」，再补能力，最后工程化。
- 公开 API 只增不删：现有 `compute_*` / `prepare_*` / `render_frame_*` 保留为薄封装，新能力走新类型，降低下游 fork 的即时破坏。
- 所有新增结构化输出走 `--json` / `--manifest`，文本格式保持兼容。

### M0 正确性（P0，优先）

#### M0-1 ink bounds（P0-1）

- `build_file.rs::SpriteSpans` 增加 `ink: Option<InkRect>`（`min_x/min_y/max_x/max_y`），在 `atlas.rs::compute_spans` 里顺带统计，零额外遍历。
- `ElementData` 增加 `ink: Option<InkRect>`；新增 `BoundsKind { Rect, Ink }`。
- 新增 `compute_ink_bounds_from_elements()`：把 ink 矩形四角按元素 2×2 矩阵变换后求并集。
  - 恒等/均匀缩放变换下与真实墨迹**精确一致**（覆盖绝大多数动画）；
  - 含旋转时是「墨迹矩形变换后的 AABB」，仍可能略大于真实像素集——必须在文档中写明，避免制造第二类静默偏差。
- `RenderedFrame` 增加 `ink_bounds: Option<BoundingBox>`：渲染完成后扫一遍 alpha 得到**精确**包围盒，供 `--meta` 使用（O(W·H)，相对合成开销可忽略）。
- CLI：`render` 增加 `--frame-bounds rect|ink`（画布取景）与 `--meta <file.json>`（逐帧精确 ink bbox + rect bbox + 坐标系说明）。
- 验收：新增金样测试，tallbird `idle_side` 40 帧 `--frame-bounds ink` 与 PNG alpha bbox 误差 ≤ 1px（当前 24.3px）。

#### M0-2 空结果退出码（P0-2）

- `list`：无 anim.bin → `Err(MissingData)`（main.rs 已统一 exit 1）；`--allow-missing` 显式放行旧行为。
- `decode`：无 .tex 同样处理，保持同类命令一致。
- `info`：**仅当 anim/build/tex 三者全空**才报错；build-only 包保持 exit 0（下游 3566 次 build symbol 普查不受影响）。这一点是对反馈原文的有意修正。
- 验收：集成测试用 `env!("CARGO_BIN_EXE_dst-anim-tool")` 断言退出码。

#### M0-3 缺失 symbol 诊断（P0-3）

- 新增 `RenderDiagnostics`：按 `(symbol, layer)` 汇总 `{frames: Vec<u32>, kind: NoSymbol | NoFrame | NoImage}`，以及 `skipped_frames`。
- `compute_*` / `prepare_*` 增加带诊断的出参版本，旧函数委托调用。
- CLI：默认把汇总打到 stderr（`missing symbol: X (N frames, layer=Y)`）；`--strict` 时返回错误、exit 1；`--meta` / `--json` 结构化输出同一份诊断。

### M1 机器接口与能力补齐（P1）

#### M1-4 按层隐藏 + 大小写归一（P1-4）

- `AnimElement` 增加 `layer_lower`（解析时生成），`disabled_elements` 匹配改用 `layer_lower`；调用方传入的集合在边界处统一 lower。
- 新增 `RenderOptions.disabled_layers: HashSet<String>`，内部统一 lower 后匹配。
- CLI：`--hide-layer <name>`（可重复）。
- 验收：高脚鸟 `beakfull`、猪人 `hat` 一条命令渲染正确。

#### M1-5 fork 能力上游化（P1-5）

- `render` 增加：`--disable-symbol`（已可复用 `BuildRef.disabled_symbols`）、`--hide-layer`、`--meta`、`--dump-elements`、`--dump-symbol`。
- dump 类统一 JSON 到 stdout，字段与库内结构同名。

#### M1-6 manifest（P1-6）

- `render --manifest <file.json>`，至少包含：
  ```json
  {
    "source": "wilson.zip",
    "bank": "wilson", "anim": "chop_loop_side",
    "frame_rate": 30.0,
    "scale": 1.0, "offset": [0.0, 0.0],
    "output_dir": "out/render",
    "frames": [{"src_index": 0, "file": "frame_000.png"}],
    "skipped": [7, 8],
    "missing_symbols": [{"symbol": "beakfull", "layer": "beak", "kind": "NoSymbol", "frames": [0, 1]}]
  }
  ```
- 下游据此替代「硬编码 30fps + glob 排序 + 帧数反推」；帧文件命名保持 `frame_{src_index:03}.png` 并在 README/manifest 中明确含义，说明 >999 帧字典序问题。

#### M1-7 批处理（P1-7）

- `render --jobs <file.json>`：一次加载 input/build/tex/atlas，循环渲染多条动画；job 内可带 `out` / `hide_layer` / `disable_symbol` / `manifest`。
- 与单条渲染共用同一核心函数，保证输出逐字节一致（加入等价性测试）。
- 库层 `prepare_*` 已是批量语义，无需改动。

#### M1-8 JSON 化的 list/info + 来源归属（P1-8）

- 新增 `load_archive_files(paths) -> Vec<(PathBuf, ParsedArchive)>`，`list`/`info` 默认改走逐文件解析（修复 `merge` 后写覆盖 + 无来源问题），`render` 继续用合并语义。
- `list --json` / `info --json`：数组元素带 `source_file`，anim/build/tex 分字段；无 anim 时 `"anim": null` 且（`list` 场景）按 M0-2 决定退出码。
- 文本模式保持旧格式，另加 `== <source> ==` 分节，避免破坏既有读者。
- 依赖：`serde` + `serde_json`，设为 `cli` feature 可选依赖，库核心不引入。

### M2 工程化（P2）

#### M2-9 feature 拆分（P2-9）

- 新增 `parallel = ["dep:rayon"]`；`cli = ["dep:clap", "dep:tracing-subscriber", "parallel"]`；`render_frame_with_elements_par` 的 cfg 改为 `feature = "parallel"`。
- 验收：`cargo check --no-default-features` 与 `--no-default-features --features parallel` 均通过。

#### M2-10 wasm 边界（P2-10）

- README 明确：字节入口（`parse_zip`/`parse_dyn`/`parse_anim_bin`/`parse_build_bin`/`ktex`/无并行 `render`）wasm 可用；`load_archives`（`std::fs`）、rayon、GUI 不可用。
- 增加 CI 检查 `cargo check --target wasm32-unknown-unknown --no-default-features`；若 `zip` 默认特性（bzip2/zstd 等 C 依赖）阻塞，则收敛为 `default-features = false, features = ["deflate"]`。

#### M2-11 fork 漂移（P2-11）

- 落地 M1-4/5 后下游可删除 `dst-render`；发布 `v0.2.0` tag 供下游钉扎，并在 README 增加「API 稳定性/破坏性变更」说明。
- 上游补金样契约测试（同 M0-1、M1-6），行为变更必须同步更新金样。

#### M2-12 CLI 细节（P2-12）

- 位置参数改命名参数：新增 `--anim` / `--out`，同时保留位置参数作为兼容回退（二者互斥、必选其一）。
- `--build` 改为可重复的 `num_args = 1`（`--build a --build b`），不再需要 `--` 分隔（兼容旧写法可后续再议）。
- `info` 多输入来源标注随 M1-8 解决；`compute_bounds_from_elements` 去重清理（`cw/ch` 与 `sw/sh` 合并为一组，非原文所称「未使用」）。
- README 增补：帧命名契约、`--meta`/`--manifest` 字段、坐标系（含 `frames` 的 y 方向）。

---

## 2. 对反馈原文的修正/保留意见

1. **`info` 不应在缺 anim.bin 时整体非零退出**：下游自身 3566 次 `info` 普查大量针对 build-only 包，若照原文执行会反噬。仅完全无数据时报错。
2. **P2-9 的代码依据在 `36cf8ff` 已不成立**：rayon 导入已 cfg 门控，纯库可编译；保留其「并行能力应与 cli 解耦」的建议。
3. **`cw/ch 未使用` 描述有误**：二者参与 center 计算，实际问题是与 `sw/sh` 重复。
4. **`--frame-bounds ink` 必须写明旋转语义**：spans 推导在中恒等变换下精确、旋转下为变换矩形 AABB；若验收要求任意变换 ≤1px，则 `--meta` 必须走渲染后扫描的精确路径。

---

## 3. 验收与测试计划

| 项 | 验收 |
| --- | --- |
| ink bounds | 金样测试：tallbird `idle_side` 与 alpha bbox 误差 ≤ 1px；旋转样例记录已知误差 |
| 退出码 | 集成测试断言 `list` 空包 exit≠0、`--allow-missing` exit=0、`info` build-only exit=0 |
| 诊断 | 构造缺 symbol 的 anim，断言 stderr 汇总、`--strict` exit≠0、manifest 字段完整 |
| manifest | 逐帧 `src_index/file` 与磁盘一致；非 30fps 动画（如 `barnacle/cooked` 24fps）字段正确 |
| 批处理 | 同一 job 批跑与逐条单跑产物逐字节一致 |
| JSON | `serde_json` 解析 + 快照测试；`source_file` 归属正确 |
| feature | `--no-default-features`、`--features parallel`、wasm check 三条命令通过 |
| 回归 | `cargo fmt -- --check` → `cargo clippy` → `cargo test` |

## 4. 建议落地顺序

1. M0（正确性三件）：ink bounds → 退出码 → 缺失诊断
2. M1-4/5（层隐藏 + CLI 能力）→ M1-6/7（manifest + 批处理）→ M1-8（JSON）
3. M2-9 feature 拆分 → M2-12 CLI 细节 → M2-10 wasm 文档/CI → M2-11 发版钉扎

---

## 5. 实施状态（2026-10-01）

已落地：

| 项 | 状态 | 说明 |
| --- | --- | --- |
| M0-1 ink bounds | 完成 | `SpriteSpans.ink` + `RenderOptions.bounds_kind` + `PreparedFrame.ink_bounds`（逐行 span 变换并集）+ `RenderedFrame.ink_bounds`（渲染后 alpha 精确扫描）；tallbird 金样测试误差 ≤2px，`--frame-bounds ink` 画布 422×839（rect 467×869） |
| M0-2 退出码 | 完成 | `list`/`decode` 缺数据非零（`--allow-missing` 放行）；`info` 仅完全空包报错，build-only 保持 0 |
| M0-3 缺失诊断 | 完成 | `RenderDiagnostics`（NoSymbol/NoFrame/NoImage + 帧位置）；stderr 汇总；`--strict` 渲染后仍写产物再 exit 1 |
| M1-4 层隐藏 | 完成 | `AnimElement.layer_lower`；`RenderOptions.with_disabled_layers` 内部 lower；CLI `--hide-layer`（大小写不敏感，已实测） |
| M1-5 CLI 能力 | 完成 | `--disable-symbol` / `--dump-elements` / `--dump-symbol` / `--manifest`(别名 `--meta`) |
| M1-6 manifest | 完成 | 含 `frame_rate`、逐帧 `src_index/file/bounds/ink_bounds`、`skipped`、`missing_symbols` |
| M1-7 批处理 | 完成 | `render --jobs jobs.json`，一次加载解码；批跑与单跑产物逐字节一致（已实测） |
| M1-8 JSON/来源 | 完成 | `load_archive_files()`；`list/info --json` 带 `source_file`；文本模式多输入加文件名分节 |
| M2-9 feature | 完成 | 新增 `parallel`；`cli` 隐含 `parallel`；`render_frame_with_elements_par` 改由 `parallel` 门控；`--no-default-features` / `--features parallel` 均通过 |
| M2-10 wasm | 完成 | `zip` 收敛为 `default-features=false, features=["deflate"]`；`cargo check --target wasm32-unknown-unknown --no-default-features` 通过；README 写明边界 |
| M2-12 CLI 细节 | 完成 | 新增 `--anim/--out`（保留位置参数回退）；README 文档化帧命名/`>999` 字典序问题；`compute_bounds` 去重 |
| M2-11 发版钉扎 | 待办 | 需要维护者打 `v0.2.0` tag 并通知下游钉扎；本次未执行任何 git 操作 |

测试：125 lib + 9 bin + 6 integration（`tests/cli.rs` 覆盖退出码、JSON、manifest、strict）。
