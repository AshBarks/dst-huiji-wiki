# 下游集成反馈：dst-auto-chess-btoy 的动画导出与重烘焙依赖评估

> **来源项目**：`dst-auto-chess-btoy`（DST 主题自走棋，Rust + wasm，发布 bilibili-toy）@ `67ed3ad` / v0.24
> **对象**：`dst-anim-tool` @ `36cf8ff`
> **日期**：2026-10-01
> **范围**：只覆盖「DST `.zip` → 逐帧 PNG → WebP 图集」的构建期导出/重烘焙链路。
> GUI、GIF 导出、`split`、`decrypt` 等下游未使用的功能不在本文范围内。
> 文末附可复现命令与观测数据。

---

## 0. 结论

| 问题 | 结论 |
| --- | --- |
| 依赖有多深？ | **构建期核心路径 100% 硬依赖，运行期 0 依赖。** `.zip` 解析、图集解码切分、元素合成渲染全部来自 dst-anim-tool，下游没有任何并行实现 |
| 用到哪些面？ | 1 个库（`dst-render` fork，直连 `archive`/`atlas`/`build_file`/`render` 公开 API）+ 2 个 CLI 子命令（`list` / `info`） |
| 规模 | 32 单位 / 127 条动画 / 2640 帧：**127 次 `dst-render` 子进程 + 约 42 次 `list`**；装备普查另跑了 **3566 次 `info`**（3029 个含 build.bin）+ 216 次 `--dump-elements` |
| 不是 100% 的地方 | 选取策略（别名/bank/build 写死）、`AnimState:Hide` 清单抽取、抽帧/缩放/锚点/打包/横向对齐/分片全部在下游自研（约 2200 行 Python） |
| 发现多少不足？ | **P0 正确性/静默错误 3 条，P1 能力缺口 5 条，P2 工程集成 4 条** |
| 最大风险 | `dst-render` 是 out-of-tree fork 且用相对路径依赖、无版本钉扎、无契约测试。上游重构 `render` 内部（如近期的 `SymbolOverrideMap` 系列提交）会**静默**破坏整条烘焙链路 |
| 一句话 | 底层能力扎实、性能足够；问题集中在**没有稳定的机器接口**（JSON/退出码/元数据）和**缺了 4 个下游刚需开关**，逼出了一个 286 行的 fork 和一整套正则解析/PNG 反推的绕行代码 |

---

## 1. 依赖地图

### 1.1 谁在用什么

管线八步见下游 `docs/ART-PIPELINE.md` §2；对应依赖如下：

| 管线环节 | 依赖方式 | 具体入口 | 不可替代性 |
| --- | --- | --- | --- |
| 1 选包 | 无（读 `anim-index.json` + 下游 `BAD_TOKENS`） | — | — |
| 2 建映射（动画名→所在包） | **CLI 子进程** | `dst-anim-tool list -i <zip>` | 低（可自己写解析），但下游不会做 |
| 3 挑名（别名/PREF） | 无（下游写死逐单位配置） | — | — |
| 4 渲染 | **Rust 库** | `dst-render` → `archive::load_archives`、`atlas::decode_atlas_images_from_tex`、`atlas::split_atlas`、`render::{BuildRef, prepare_animation_frames_with_overrides, render_frame_with_elements}` | **高：本仓无任何替代实现** |
| 5 锚点/地面线 | 无（PIL 读渲染 PNG 的 alpha bbox） | — | — |
| 6 抽帧/缩放 | 无 | — | — |
| 7 打包 WebP / sprites.json | 无 | — | — |
| 8 报表 | 无 | — | — |
| （旁路）build symbol 普查 | **CLI 子进程** | `dst-anim-tool info -i <zip>` × 3566 | 中；无 JSON，靠正则 |
| （旁路）装备挂点普查 | **fork CLI** | `dst-render --dump-elements` | 官方 CLI 完全没有 |
| （旁路）隐藏层清单 | 无（从 DST Lua 抽 `scrapbook_hide`） | — | — |

下游调用点（`tools/art/`，共 2188 行 Python，4 个脚本直接调工具）：

- `bake_spike.py`：`list` + `dst-render`（烘焙主链路）
- `scan_symbols.py:28-35`：正则解析 `info` 输出
- `symbol_isolation.py:29-40`：正则解析 `info` 输出 + 反复调 `dst-render`
- `anchor_census.py:69`：`dst-render --dump-elements`

### 1.2 依赖强度：导出 vs 重烘焙

| 场景 | 是否依赖 | 说明 |
| --- | --- | --- |
| 全量导出（游戏 `.zip` → 图集） | **100% 依赖** | 解析/解码/合成全部来自工具；工具缺位则管线停摆 |
| 全量重烘焙（改抽帧/分辨率/隐藏层后重跑） | **100% 依赖** | 必须重新调 `list` + `dst-render` |
| 产物返修（不改像素，只改元数据） | 0 依赖 | `recenter_x.py`（横向对齐）、`split_units.py`（按单位分片）、`repack_tight.py`（逐帧紧裁）、`strike.py`（打击帧）都在现有产物上做 |
| 运行期（wasm / 页面里） | **0 依赖** | 走预渲染 WebP 路线，工具不进发布包；故障只在构建期爆炸 |

> 故障半径：构建期强依赖 + 运行期零依赖 = 「工具坏了只是没法出新内容/重烘，不会线上崩」。

### 1.3 dst-render：为补 4 个缺口而存在的 fork

`dst-auto-chess-btoy/tools/dst-render/src/main.rs`（286 行）是官方 `render` 的增强封装，自述动机见其文件头注释（`main.rs:1-14`）：

| 增强 | 官方 CLI | 官方库 API | fork 怎么做 |
| --- | --- | --- | --- |
| `--disable-symbol` | 无 | `disabled_symbols` 存在于 `BuildRef`，但 CLI 不透出 | 传 `BuildRef.disabled_symbols` |
| `--disable-layer`（对应 `AnimState:Hide`） | 无 | **没有**按层名接口，只有 `(symbol, layer)` 精确匹配 | 自己克隆并预过滤 `anim.frames[..].elements`（`main.rs:148-168`） |
| `--meta`（逐帧包围盒 JSON） | 无 | `PreparedFrame.bounds` 是公开字段，可自取 | 直接读 `render::prepare_animation_frames_with_overrides`（`main.rs:232-278`） |
| `--dump-symbol` / `--dump-elements` | 无 | 无对应导出 | 遍历 `BuildFile.symbols` / `AnimFrame.elements` 自行序列化 |

代价：`cmd_render` 的核心逻辑（加载 skit/build、动画定位、逐帧渲染）被复制了一份（对照 `src/cli.rs:251-398`），并直接耦合 `render.rs` 内部函数签名。上游任何签名调整都会让 fork 编译失败——这是好事（编译期暴露）；但**行为级变化**（如 `prepare_*` 的 bounds 语义、`find_symbol_frame` 的查找顺序）则无声无息。

### 1.4 版本与升级耦合

- 依赖声明：`dst-anim-tool = { path = "../../../dst-anim-tool", default-features = false, features = ["cli"] }`（`tools/dst-render/Cargo.toml:11`）。
- 没有 rev/tag 钉扎，没有 vendoring；上游 HEAD 直接进入下游构建。
- 下游没有任何针对 `dst-render` 的自动化冒烟测试（`cargo xtask` 只校验最终产物）。
- 上游近期仍在活跃开发（`--symbol-map`、符号重映射、hash 查表告警等），漂移风险实时存在。

---

## 2. 不足清单

严重度定义：
**P0** = 会产生错误结果或被静默吞掉的错误；**P1** = 缺能力，迫使下游 fork/绕行；**P2** = 工程与集成体验。

### P0-1 `bounds` 不是墨迹包围盒，不能用来做锚点

**现象**：`--meta`（以及库的 `BoundingBox`）是「元素贴图矩形并集」，不是渲染后实际不透明像素的包围盒。

- 代码依据：`src/render.rs:297-340` 用 `elem.sprite.width()/height()` 的四角变换算包围盒，不检查 alpha；元素即使完全透明或缩放到 0，只要在元素列表里就参与计算。
- 实测（2026-10-01，tallbird `idle_side`，40 帧）：把 PNG alpha bbox 映射回同一坐标系后，**`--meta` 的 bottom 比实际墨迹低 24.3px（中位数），top 高 5.1px**。下游 Spike A 在别的单位上量到 **40~120px 且各单位不一致**（`docs/archive/SPIKE-A.md` 坑三）。
- 影响：DST 原画的贴图矩形带大量透明边距，误差随单位/动作变化。若拿它当脚底基准，单位会整体下沉且各单位不齐。下游被迫**对每一帧 PNG 用 PIL 重新求 alpha bbox**（`bake_spike.py:308-317`）——等于把工具已经算过一遍的东西在体外重算，且必须保留全部中间 PNG。
- 建议：提供 ink bounds，例如 `compute_ink_bounds_from_elements` / `--frame-bounds ink`，或让 `--meta` 至少带 `alpha_bottom`；并在文档里明确现有 `BoundingBox` 的语义是 rect union。

### P0-2 `list` / `info` 空结果退出码为 0（静默成功）

**现象**：对没有 `anim.bin` 的包，`list` 打印一句 stderr 后**正常退出**：

```text
$ dst-anim-tool list -i ".../data/anim/pig_build.zip"
no anim.bin found
$ echo $?
0
```

- 代码依据：`src/cli.rs:400-412`，`else { eprintln!("no anim.bin found"); }`，然后 `Ok(())`；`info` 同理（`cli.rs:414-433` 打印 `anim.bin: not found` 后继续）。
- 影响：下游 `decode_zip_anim_list`（`bake_spike.py:152-155`）只收集 stdout 里带 `/` 的行，不检查退出码。一个包路径写错/包类型不对，会被解释为「这个包里没有动画」，进而变成「该动作不在候选包里」的误导性结论。这类错误在 127 次调用里很难人工发现。
- 建议：无 anim.bin 时以非零退出码结束（或用 `--allow-missing` 显式放行）；配合 P1-5 的 JSON 输出给出结构化状态。

### P0-3 找不到 symbol 时静默丢元素

**现象**：`find_symbol_frame` 返回 `None` 时，`compute_frame_elements_with_overrides` 直接 `continue`（`src/render.rs:267-289`），不计数、不告警、不影响退出码。

- 触发场景：`--build` 选错包、build 与 anim 版本不匹配、symbol 改名、大小写/后缀差异。
- 影响：渲染结果缺胳膊少腿但命令成功。下游 Spike A 的高脚鸟「嘴上有两个紫色球」排查了两轮，本质就是元素级异常的可见性太差。现在靠 `--dump-elements` 人工比对才能定位。
- 已有先例：上游 `503b991` 已为 build hash 查不到的情况加了 `tracing warn`。symbol 级缺失值得同样处理。
- 建议：stderr 汇总 `missing symbol: <name> (N frames, layer=...)`，或在 `--strict` 下报错。

### P1-4 库/CLI 均无「按层名隐藏」接口

DST 的隐藏语义是按**层名**：`inst.AnimState:Hide("beakfull")`（`prefabs/tallbird.lua:182`）、`AnimState:Hide("hat")`（`prefabs/pigman.lua:598`）。

- 库现有接口只有 `disabled_elements: HashSet<(symbol_lower, layer_name)>`（`src/render.rs:230-261`），要求调用方先枚举每个层下的所有 symbol 才能构造集合。
- 官方 CLI 连 `disabled_symbols` 都没有透出。
- 下游只能 fork：`dst-render` 克隆整棵 `animation.frames`，按 `layer_name` 过滤后再交给渲染（`main.rs:148-168`）。这既多一次全量 `Vec` 复制，也绕开了库的 override/诊断路径。
- 顺带一个 API 坑：库内匹配 `layer_name` 不做大小写归一（`render.rs:255-258`），而 DST 层名大小写混用（如 `ARM_carry_up`、`HAIR_HAT`），调用方必须自己 lower 两遍。
- 建议：`compute_*` / `prepare_*` 增加 `disabled_layers: &HashSet<String>`（内部统一 `to_lowercase`），CLI 加 `--hide-layer`。

### P1-5 刚需能力只在 fork 里，官方 CLI 没有

| 能力 | fork 位置 | 下游用途 |
| --- | --- | --- |
| `--disable-symbol` | `main.rs:139-144, 227-240` | 合并多 build 时混入的帽子/蛋/雕像 symbol |
| `--disable-layer` | `main.rs:148-168` | P1-4 |
| `--meta`（逐帧 bbox JSON） | `main.rs:84-91, 243-278` | 帧索引对齐（bounds 本身不可用，见 P0-1） |
| `--dump-elements` | `main.rs:204-225` | 装备挂点普查（`anchor_census.py`）、层/符号定位 |
| `--dump-symbol` | `main.rs:171-202` | 诊断「多余图案是哪个 symbol」 |

这些能力**全部**是通用的 DST 调试/集成需求，且都在 `render.rs` 已有内部数据结构支持，上游化成本很低。建议直接并入官方 `render`（命名可参考 fork，或归入统一的 `--json` 元数据开关）。

### P1-6 渲染不输出时序与逐帧元数据

- `AnimAnimation.frame_rate` 已解析（`src/anim.rs:36`），但 `cmd_render` 完全丢弃；`BuildFrame.duration`（`src/build_file.rs:35-37`）只用于 `frame_for_anim_frame` 内部回查，也不出现在输出里。
- 输出只有 `frame_{i:03}.png`（`src/cli.rs:376-396`），没有 manifest：不告诉调用方一共有几帧、哪些源帧被跳过（元素全缺失时 `None` 会形成空号）、每条动画多少 fps。
- 下游只能**硬编码 30fps**、用「抽后帧数 / 原始帧数」反推每帧停留（`bake_spike.py:108,167-173,396-398`）。如果某条动画的 `frame_rate != 30`，循环速度会静默出错；任何依赖 checksum 的重烘都复现不了这条信息。
- 建议：`render` 增加 `--manifest out.json`，至少包含 `{anim, frame_rate, frames: [{src_index, file}], skipped: [...]}`；或让 `--meta` 带上 fps/逐帧 delay。

### P1-7 无批处理入口，127 次进程反复解码同一 build

- 现状：一次进程只能渲染一条 `<bank>/<anim>`，每次都要重新 `load_archives` + `decode_atlas_images_from_tex` + `split_atlas`（`src/cli.rs:300-347`）。Phase 1 烘焙 127 条动画就是 127 次完整启动，其中大量动画共享同一个 build 包（32 个单位、42 个 anim 包）。
- 工具本身单次渲染很快（下游记录 9 帧 49ms），但 2640 帧 + 上百次图集解码/切分/进程启动是浪费；`bake_spike.py` 只能靠并发度硬扛。
- 建议：`render --manifest jobs.json`（多动画、一次加载 build），或接受多个 `anim_path` + 统一输出根目录。库层 `prepare_*` 已经是批量语义，缺的只是 CLI 入口。

### P1-8 `list` / `info` 没有机器可读输出，也没有来源归属

- 输出是给人看的文本，下游用正则解析：`scan_symbols.py:28-29` 匹配 `symbol 'x': N frames` / `name: xxx`；`symbol_isolation.py` 靠 `symbol '` 前缀切字符串。上游改一个空格/缩进，下游普查结果全部为空且不报错。
- `-i a.zip b.zip` 会把多个输入 `merge`，但 `ParsedArchive::merge` 对 anim/build 是**后写的直接覆盖前者**（`src/archive.rs:40-54`），而 `info` 只打印合并后的单份，**不标注来自哪个文件**。下游要做「动画名→所在包」映射时只能一个包一个进程地调，这是 42 次 `list` 的根本原因。
- 建议：`list --json`、`info --json`，字段带 `source_file`；`render` 的错误也走结构化输出。

### P2-9 feature 划分：想用 render 就必须开 `cli`

- `render.rs:4` 无条件 `use rayon::prelude::*`，而 `rayon` 只在 `cli` feature 下作为依赖存在（`Cargo.toml`）。于是纯库消费者必须 `default-features = false, features = ["cli"]`，顺带拖入 `clap` + `tracing-subscriber`——`dst-render` 的依赖声明就是被迫这么写的。
- 建议：拆出 `render` feature（rayon + image 即可），或把 `rayon` 提为非可选基础依赖。

### P2-10 wasm 目标没有保障

- 下游曾评估「运行期在 wasm 里合成动画」（`docs/reference/SIBLING-PROJECTS-AND-ASSETS.md:992` 明确列为未验证项）。最终选了预渲染路线，所以**不阻塞本项目**，但结论需要上游背书，否则每个下游都要重新试错。
- 现状证据：`archive::parse_zip`（`src/archive.rs:57`）是纯字节入口，wasm 友好；但 `load_archives` 用 `std::fs`（`archive.rs:131-160`），`render` 依赖 rayon，没有 `wasm32-unknown-unknown` 的 CI/构建说明。
- 建议：在 README 明确「不支持 wasm/仅 `parse_zip` 路径可用」，或加一个 wasm 目标编译检查。

### P2-11 fork 漂移与契约测试缺失（下游侧问题，需两边共同解决）

- 相对路径依赖 + 无钉扎 + 无冒烟测试，意味着上游一次「行为改进」可能让图集静默变化（最坏情况：尺寸/锚点全偏，直到有人肉眼发现）。
- 建议上游化 P1-4/P1-5 后瘦身 fork；在完成前，下游应把 `dst-render` 的一次「单元渲染 + 像素/尺寸断言」纳入构建闸门（目前只有产物预算闸门）。

### P2-12 CLI 设计细节

| 项 | 现象 | 建议 |
| --- | --- | --- |
| `--build` 变长参数 | 与位置参数 `anim_path` / `output_dir` 相邻，必须 `--` 分隔（README 有写；下游硬编码 `--`，`bake_spike.py:297`） | 位置参数改命名参数 `--anim` / `--out` 更符合脚本预期 |
| 帧文件命名契约 | `frame_{i:03}.png` 的 `i` 是源帧索引，空号含义未文档化；无 manifest；>999 帧时字典序排序会错 | 文档化 + manifest（同 P1-6） |
| `disabled_elements` 大小写 | 库不归一化 layer（`render.rs:255-258`） | 内部统一 lower |
| `info` 多输入合并 | 后写覆盖（`archive.rs:40-54`），静默丢第一份的 build | 明确报错或分来源输出 |
| `compute_bounds_from_elements` | `cw/ch` 未使用、`sw/sh` 与之重复（`render.rs:307-316`） | 顺手清理 |
| GUI/GIF 与 CLI 的能力差 | `symbol_map` 库层已支持，CLI 近版本才补上；`disabled_*`/dump 仍缺 | 保持 CLI 与库能力对齐 |

---

## 3. 下游现有规避措施（这些就是改进的价值）

| 上游缺口 | 下游绕行 | 代价 |
| --- | --- | --- |
| P0-1 bounds 不可用 | 保留全部渲染 PNG，用 PIL 对每帧求 alpha bbox（`bake_spike.py:308-317`） | 额外磁盘/IO；`--meta` 只用来拿帧索引 |
| P0-2/P1-8 文本接口 | 正则解析 `list`/`info`，三重校验退出码+文件+meta（`bake_spike.py:300-303`） | 上游格式改动即全断 |
| P1-4 无层隐藏 | `dst-render` 克隆并过滤 `anim.frames` | fork + 全量 Vec 复制 |
| P1-5 无 CLI 开关 | fork 里复制 `cmd_render` 逻辑 | 286 行重复代码 + 内部 API 耦合 |
| P1-6 无时序 | 硬编码 30fps + 帧数重算 + 防频闪下限 | `frame_rate != 30` 时静默变速 |
| P1-7 无批处理 | 127 次子进程 + 并发 | 重复解码/切分 build 图集 |
| 多 build 污染（帽子/雕像） | 显式逐单位配置 build_zip + `BAD_TOKENS` 黑名单（`bake_spike.py:113-117`、`tools/data/art.spec.json`） | 32 个单位 914 行配置，人工维护 |
| 找不到 symbol 静默 | `--dump-symbol` / `--dump-elements` 人工排查 | 排查成本高，见 Spike A 高脚鸟两轮返工 |

---

## 4. 建议的落地顺序

| 优先级 | 改动 | 验收标准 |
| --- | --- | --- |
| P0 | `list`/`info` 无数据时非零退出；或 `--allow-missing` | `list -i <build-only.zip>` 退出码 != 0 |
| P0 | ink bounds（库 API 或 `--frame-bounds ink`） | 与 PNG alpha bbox 误差 <= 1px，跨帧可比 |
| P0 | symbol 缺失汇总告警（`--strict` 可升级为错误） | stderr 报出 symbol/layer/帧数 |
| P1 | `--hide-layer` + `disabled_layers` 参数（大小写不敏感） | 高脚鸟 `beakfull`、猪人 `hat` 一条命令渲染正确 |
| P1 | fork 的 `--disable-symbol` / `--meta` / `--dump-elements` 上游化 | 下游可删除 `dst-render` |
| P1 | `render --manifest`（帧索引/时序/skip/输出路径） | 下游不再硬编码 30fps，不再 glob 排序 |
| P1 | `list`/`info --json`（带 `source_file`） | 下游删除全部正则 |
| P1 | `render --manifest` 批量多动画 | 127 次进程降到个位数，build 只解码一次 |
| P2 | `render` feature 拆分（rayon 不绑 `cli`） | `default-features = false, features = ["render"]` 可编译 |
| P2 | README 声明 wasm 支持边界 | `cargo check --target wasm32-unknown-unknown` 的明确结论 |
| P2 | CLI 位置参数改命名参数（`--anim`/`--out`） | 不再需要 `--` 分隔 |

---

## 5. 复现记录（2026-10-01，Linux）

```sh
DST="$HOME/.steam/debian-installation/steamapps/common/Don't Starve Together/data/anim"
TOOL="<repo>/dst-anim-tool/target/release/dst-anim-tool"      # 或下游 .spike/target/release/dst-anim-tool
DST_RENDER="<repo>/dst-auto-chess-btoy/.spike/target/release/dst-render"
```

1. **P0-2 静默成功**

   ```sh
   "$TOOL" list -i "$DST/pig_build.zip"; echo "exit=$?"
   # no anim.bin found
   # exit=0
   ```

2. **P0-1 bounds ≠ 墨迹**（tallbird `idle_side`，40 帧）

   ```sh
   "$DST_RENDER" -i "$DST/ds_tallbird_basic.zip" --meta /tmp/meta.json \
                 -- tallbird/idle_side /tmp/frames
   ```

   把 `--meta` 的并集包围盒映射回渲染用的 union 原点后逐帧比较：

   - `--meta` 并集：top=-791.3，bottom=72.1（高 863.4）
   - PNG alpha 墨迹（映射到同一坐标系）：高约 835
   - **meta.bottom 比实际墨迹最低点低 24.3px（40 帧中位数，极差 0.1px）**；meta.top 比实际高 5.1px
   - 这与下游 `docs/archive/SPIKE-A.md` 坑三记录的 40~120px（因单位而异）一致

3. **规模数据来源**

   - `tools/data/art.spec.json`：32 单位 / 128 个动作位 / 127 条实际动画 / 42 个 anim zip / 32 个 build zip
   - `.spike/bake-out.log`：Phase 1 实烘 32 单位 / 2640 帧 / 6 图集
   - `docs/reference/build-symbols.json`：3029 个含 build.bin 的包（由 3566 次 `info` 得出）
   - `docs/reference/equipment-anchors.json`：72 单位 × 3 动作 `--dump-elements`

---

## 6. 关联文档

- 下游管线全貌：`dst-auto-chess-btoy/docs/ART-PIPELINE.md`
- 下游八步实现：`dst-auto-chess-btoy/tools/art/bake_spike.py`
- 本工具被复用方式与 wasm 评估：`dst-auto-chess-btoy/docs/reference/SIBLING-PROJECTS-AND-ASSETS.md` §B
- 相关设计：本仓 `docs/BevyIntegration.md`（运行期集成的另一类消费者，同样会需要 P1-6/P1-8 的元数据）
