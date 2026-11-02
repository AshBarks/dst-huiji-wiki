# 动画跟随 FX（FollowSymbol）合成预览

本文记录 `Follower:FollowSymbol` 挂点 FX 的机制、`anim-index` 数据契约、
渲染合成算法与 WebUI 行为。是 [ANIM_SKIN_PREVIEW_PLAN.md](ANIM_SKIN_PREVIEW_PLAN.md)
§8.1 的延伸。

## 1. 游戏里的机制

一个 prefab 的完整外观可能由**多个实体**组成。以 `shadowthrall_wings` 为例：

```lua
-- prefabs/shadowthrall_wings.lua
local cape = CreateCapeFx()          -- AddAnimState / AddFollower
cape.AnimState:SetBank("shadow_thrall_wings")
cape.AnimState:SetBuild("shadow_thrall_wings")
cape.AnimState:PlayAnimation("fx_cape_front", true)

cape.entity:SetParent(inst.entity)
cape.Follower:FollowSymbol(inst.GUID, "cape_front_swap", nil, nil, nil, true)
```

- 主实体的动画里，`cape_front_swap` **没有 build 贴图**，但每帧带有完整的
  变换矩阵（`anim.bin` 的 element `matrix` + `translation`）。
- 引擎每帧把跟随实体的 Transform 贴到该 symbol 的变换上（位置 + 旋转 +
  缩放），跟随实体保留自己的 bank/build/animation。
- 因此这类 symbol 在预览里"找不到 provider build"是**正常现象**，正确
  补法是合成，而不是找 build。

数值验证：`shadow_thrall_wings/scrapbook` 是 Klei 烘焙好的合成动画，
其元素变换满足 `world = A·local + T`（A/T 为挂点变换），误差 < 0.001。

### 三类"无同名 provider 的 symbol"

| 类别 | 例子 | 正确补法 |
|---|---|---|
| 跟随挂点 | `cape_front_swap`、`cb_hair_follow`、`swap_fire` | 合成跟随动画（本文） |
| 运行时替换占位 | `swap_hat`、`swap_food`、`swap_plate` | `anim-remap-index` / 手动 build |
| 无意义残留 | `bounding`、`column1` | 忽略 |

WebUI 现在按"是否有 provider / 是否有 FollowSymbol 记录 / 名字是否像挂点"
区分：只有无 provider 的才进入挂点 UI，避免把 `swap_hat` 这类正常替换
symbol 误判成挂点。

## 2. 数据流

```
Lua 源码
  └─ src/parser/follow_symbol.rs        实体级追踪 + 工厂/字段/同文件 SpawnPrefab
       └─ anim-index.json               PrefabAnimRecord.follow_fx[]
            └─ /api/anim/assets/prefab/:file
                 └─ anim_assets.js      refreshFollowSpecs()（Lua 索引优先，
                                        缺失时用同 bank 启发式候选）
                      └─ /api/anim/assets/(preview|render)?follows=[JSON]
                           └─ preview::render_animation_frames
                                └─ dst-anim-tool render 原语 → 合成帧
```

### 2.1 Lua 提取（`src/parser/follow_symbol.rs`）

一条记录：

```json
{
  "anchor": "cape_front_swap",
  "bank": "shadow_thrall_wings",
  "build": "shadow_thrall_wings",
  "animation": "fx_cape_front",
  "offset": [0, 0],
  "frame_begin": null,
  "frame_end": null,
  "final_offset": null,
  "mult_colour": null,
  "add_colour": null,
  "sync_frame": false,
  "random_frame": true,
  "receiver": "cape",
  "spawn_prefab": null,
  "confidence": "resolved",
  "prefab": "shadowthrall_wings"
}
```

- **实体级追踪**：按实体路径（`fx`、`inst`、`inst.head`、`inst.swapfx2`）
  记录 `SetBank/SetBuild/PlayAnimation`、`SetFinalOffset`、字面量
  `SetMultColour`/`SetAddColour`、`SetFrame(math.random(...))`；
- **绑定解析**：`local cape = CreateCapeFx()`、`inst.head = CreateHead()`、
  `inst.swapfx = SpawnPrefab("x")`；同文件 `SpawnPrefab` 通过
  `Prefab("x", fn, ...)` 反查 `fn` 的实体配置；
- **参数化工厂**：`AddFollowSymbol(inst, sym, anim)` 在调用点取实参；
- **字符串常量 / `..` 拼接**；
- **跨文件 `SpawnPrefab`**：receiver 绑定到 `SpawnPrefab("x")` 但 `x`
  定义在别的文件时，记录 `spawn_prefab = "x"`、`animation = null`；
  由 `anim-index` 的第二遍全局回填补全（见 §2.3）。
- **Unresolved**：动态 `SpawnPrefab(expr)`、状态图调用、动态 animation。
  记录仍保留（`animation` 缺失），前端提供手动选择。

`anim-index` 顶层新增 `follow_index_version`（当前 2）；WebUI 读到 `< 2`
（旧产物）时会提示重跑 `anim-index`。字段为增量扩展，`schema_version`
不变；旧产物读取时 `follow_fx` 为空，前端自动回退到启发式。

### 2.2 跨文件 `SpawnPrefab` 回填（`anim-index` 第二遍）

`parse_follow_symbols_and_entities` 在同一次 AST 遍历里额外产出本文件每个
`Prefab("x", fn, ...)` 的静态默认值（`bank` / `build` / `animation` /
`SetFinalOffset` / 字面量颜色 / 随机帧）。`anim-index` 汇总成全脚本的
`prefab → defaults` 表，再用 `resolve_cross_file_follows` 回填所有
`spawn_prefab` 指向它的未解析记录：

- 只补记录里为空的字段，调用点显式设置的值优先；
- 回填成功即置 `confidence = resolved`；
- 统计写入 `stats.follow_cross_resolved`（按
  `(prefab_file, anchor, spawn_prefab, receiver)` 去重）；
- 表构造器绑定（`inst.frostbreath = { fx = SpawnPrefab("frostbreath") }`）
  会展开为 `inst.frostbreath.fx`，因此嵌套 receiver 也能命中。

动态 `SpawnPrefab(fxname)`、定义文件里动画名本身是表达式（如
`"idle"..tostring(i)`）的情况仍保持未解析。

### 2.3 启发式候选（后端）

`animation_info` 对**没有 build provider** 的 symbol，在同文件同 bank 的
动画里找"自身所有 symbol 都有贴图"的动画作为候选，按名字打分：

| 关系 | 分数 |
|---|---|
| 动画名 == `symbol`（去掉 `_swap`） | 100 |
| 动画名 == `fx_` + base | 95 |
| 名字互相包含 | 60 |
| `fx_` 前缀 | 40 |
| `_loop` 后缀 | 30 |

返回 `follow_candidates: {symbol: [{bank, animation, file, score, reason}]}`。
Lua 索引存在时以前者为准，启发式只作兜底/候选展示。当前 `anim-index`
中 188 条去重关系有 113 条可解析（80 resolved + 33 static，其中 3 条由
§2.2 跨文件回填），其余 75 条保留为未解析记录。

未解析里相当一部分**不是精灵 FX**：`eyeflame`、`torchfire`、`character_fire`、
`thurible_smoke`、`squideyelight` 等只有 `VFXEffect`/`Light`，没有
`AnimState`，无法用"精灵 × 挂点矩阵"合成；另一类是动态
`SpawnPrefab(expr)` 或动态 `PlayAnimation`（如 `makefn("collapse_small")`、
`"idle"..tostring(i)`）。两类都需要不同的补法，不应混在精灵合成里。

## 3. 渲染合成

### 3.1 变换与着色

`ElementData` 的渲染约定是 `world = t + M·p`，`M = [[a, c], [b, d]]`。设挂点
变换 `(A, T)`、跟随元素本地变换 `(L, t)`：

```
M' = A · L
t' = T + A · t + offset
```

`offset` 为 Lua 传入的 x/y（动画单位）。实现见 `render.rs::AnchorTransform::compose_element`。

在变换之后应用字面量颜色：

- `SetMultColour(r,g,b,a)`：`rgb' = rgb * mult.rgb`，`alpha' = alpha * mult.a`；
- `SetAddColour(r,g,b,a)`：`rgb' = rgb + add.rgb`（归一化 0..1 后加色）。

颜色作用于合成后的贴图副本；alpha 变化时同时丢弃预计算的行跨度，避免
"透明像素按不透明快速拷贝"的错误。

### 3.2 z 序

`anim.bin` 的元素按 `z_index` 升序排列；渲染器按列表顺序绘制。合成时把
跟随元素以挂点 `z_index` 为 key 插入，稳定排序保证：

- 与挂点同 z 的父元素先绘制，跟随 FX 后绘制（对齐引擎"子实体在父之后"）；
- 跟随动画内部顺序不变；
- `SetFinalOffset(n)` 映射为 `z = 挂点 z + n * 1000`：`+1` 整体画在父元素
  之前、`-1` 整体之后，`0` 保持在挂点位置。

`shadow_thrall_wings` 的挂点 z 与官方 `scrapbook` 的烘焙顺序一致。

### 3.3 帧映射

- 默认跟随帧 `j = (主帧 + phase) mod 跟随帧数`；
- `sync_frame`（`FollowSymbol` 第 7 参数，推断）为真时 `j = 主帧 mod n`；
- `frame_begin/frame_end` 是**主动画帧窗口**（含端点），窗口外不挂；
- 挂点在某一帧不存在（例如 `appear` 里披风还没出现）时跳过该帧，计入
  `FollowReport.anchor_missing_frames`，不算错误。

帧窗口的"主动画帧"解释由数据反证（无需引擎）：

| 调用点 | 跟随动画帧数 | 传入窗口 | 若解释为跟随者自身帧 |
|---|---|---|---|
| `wx78_common.lua` | `steam_1/2` = 28 帧 | `0`、`1`、`2..5` | 只显示 1 帧，不合理 |
| `winona_telebrella.lua` | fx prefab `swap1` = 3 帧 | `0..19` | 3 帧动画不可能有 0..19 |
| `voidcloth_scythe.lua` | `swap_loop_*` = 30 帧 | `0..2`、`5..`、`6..`、`7..` | 窗口恰好对应挥砍目标帧 |

### 3.4 build 解析

跟随动画可能来自其他 zip/build：

- `load_follow_archives` 解析 spec 指向的归档（默认主文件），按需追加其
  build；缺失 build 通过 `archive_index` 全树查找后加载；
- 每个挂点使用**限定 build 列表**查找贴图，避免与主 build 同名冲突；
- `spec.bank` 为空时回退到主动画 bank。

## 4. HTTP / 前端

### 请求

`/api/anim/assets/preview` 与 `/api/anim/assets/render` 新增查询参数
`follows`（URL 编码的 JSON 数组）：

```json
[{"anchor":"cape_front_swap","bank":"shadow_thrall_wings",
  "animation":"fx_cape_front","file":"shadow_thrall_wings.zip",
  "build":null,"offset":[0,0],"phase":0,
  "frame_begin":null,"frame_end":null,
  "final_offset":null,"mult_colour":null,"add_colour":null,
  "sync_frame":false,"random_frame":true}]
```

### 响应

`preview` 额外返回：

```json
"follows": [{"anchor":"cape_front_swap","bank":"...","animation":"fx_cape_front",
             "file":"...","attached_frames":40,
             "anchor_missing_frames":[],"follower_empty_frames":[],
             "sync_frame":false,"random_frame":true}],
"follow_errors": [{"anchor":"...","animation":"...","reason":"animation not found: ..."}]
```

### 前端

- Symbol Dependencies 里挂点显示 `anchor` 徽标、跟随动画名、来源与修饰徽标
  （同步帧 / 随机起始帧 / 前后偏移 / 染色）；
- 跟随动画下拉可手动换成任意同会话动画，或"不挂接"；
- 每个挂点有相位输入框与"随机"按钮；`sync_frame` 为真时相位禁用并提示；
- "补齐缺失 Symbol"不再扫描挂点；
- 预览下方显示 `跟随 FX N 个 · 已合成`，`follow_errors` 红字列出；
- `follow_index_version < 2` 时在 Symbol Dependencies 顶部提示索引过期。

## 5. 已知限制

- 动态 `SpawnPrefab(expr)`、状态图内的 `FollowSymbol` 未解析；
- 跨文件回填依赖被引用 prefab 在 `prefabs/` 下的静态 `fn()` 配置；若
  `PlayAnimation` 本身是表达式（拼接/状态切换），该记录仍保持未解析；
- 同一函数内分支里的多个 `PlayAnimation` 采用"最后赋值"近似：例如
  `tophat_shadow_fx` 的 `swirl_attachfn` front 分支会取到 else 分支的
  `swirl_back`，可在 UI 手动改；
- `FollowSymbol` 第 7 参数（`useframe`）只能按"帧同步"推断实现（全脚本
  5 处），未做引擎级验证；
- 游戏里的随机起始帧无法复现，只提供相位/随机按钮；
- `SetSymbolLightOverride` / `SetSymbolBloom` 依赖场景光照与泛光，未实现；
  `SetAddColour` 只处理字面量，`colouraddersync` 的运行时颜色不支持；
- 多层嵌套跟随（跟随者自身再挂跟随者）只合成一层；
- 旧 `anim-index.json` 没有 `follow_fx`/`follow_index_version`，需重跑
  `anim-index`（WebUI 会提示），否则依赖启发式候选。

## 6. 测试

- `crates/dst-anim-tool`：合成公式（与 `scrapbook` 烘焙值对齐）、偏移、
  z 序插入、`SetFinalOffset` 前后偏移、颜色乘/加、挂点查找；
- `src/parser/follow_symbol.rs`：本地工厂、参数化工厂、内联实体、字段绑定、
  同文件/跨文件 `SpawnPrefab`（含表构造器绑定与 `resolve_cross_file_follows`）、
  字符串常量、offset/帧窗口、颜色/最终偏移/随机帧/同步帧、动态 animation
  的 unresolved 分支；
- `src/scripts_sync/anim/preview.rs`：候选打分、JSON 契约（含新字段）；
  `#[ignore]` 真实数据用例合成 `shadow_thrall_wings/idle_side` 并断言 3 个
  挂点均有帧被挂接（`cargo test -p dst-huiji-wiki --lib -- --include-ignored`，
  需 `DST__ROOT`）。
