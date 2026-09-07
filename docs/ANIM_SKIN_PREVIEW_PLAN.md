# 动画皮肤（.dyn）预览集成方案（草案）

> 目标：把皮肤动画资源接入现有 WebUI 动画预览流程。
> 当前状态：探索与方案设计，不做完整实现。

## 1. 结论

皮肤资源的关联关系已经很明确，且 `dst-anim-tool` 已经具备解析与渲染能力：

- `prefabs/skinprefabs.lua` 提供 `skin -> base_prefab -> build_name` 的映射；
- `data/anim/dynamic/*.zip` 提供皮肤 build（`build.bin`）；
- `data/anim/dynamic/*.dyn` 提供皮肤贴图（`.tex` atlas）；
- `.zip` 与 `.dyn` 通过同名 stem 成对使用；
- `dst-anim-tool` 已有 `--skin` 渲染支持，内部会自动寻找 `.dyn` / `.zip` 伴随文件。

因此下一步不需要从零做皮肤系统，只需要把“prefab -> skins”关系引入现有动画预览页，并在渲染参数中把 skin 包传给后端。

## 2. 关键数据源

### 2.1 `prefabs/skinprefabs.lua`

这是皮肤与 prefab 的主要关联源。

示例：

```lua
table.insert(prefs, CreatePrefabSkin("abigail_ice", {
    base_prefab = "abigail",
    type = "item",
    build_name_override = "abigail_ice",
    ...
}))
```

要点：

- `skinprefabs.lua` 中每个 `CreatePrefabSkin(...)` 都是一个皮肤条目；
- `base_prefab` 是该皮肤对应的原始 prefab；
- `build_name_override` 可选，表示皮肤 build 名；
- 如果没有 `build_name_override`，通常可以直接使用 skin 名作为 build 名；
- 一个 build 可能被多个 skin prefab 使用，例如 `abigail_flower_ice` 和 `abigail_ice`。

### 2.2 `data/anim/dynamic/*`

皮肤的动画资源在这里：

```text
dynamic/abigail_ice.zip
dynamic/abigail_ice.dyn
```

其中：

- `dynamic/<skin>.zip` 通常是 build-only 包，内含 `build.bin`；
- `dynamic/<skin>.dyn` 通常是贴图包，内含 `atlas-*.tex`；
- 两者通过同名 stem 配对。

### 2.3 `skin_assets.lua`

`skin_assets.lua` 中成对出现：

```lua
Asset("DYNAMIC_ANIM", "anim/dynamic/abigail_ice.zip"),
Asset("PKGREF", "anim/dynamic/abigail_ice.dyn"),
```

这进一步确认了 `.zip` 与 `.dyn` 的伴生关系。

## 3. 实测验证

### 3.1 文件配对规模

当前游戏树中：

| 项目 | 数量 |
|---|---:|
| `dynamic/*.dyn` | 3439 |
| `dynamic/*.zip` | 3318 |
| 可配对的 `zip + dyn` | 3318 |

`skin_assets.lua` 中：

| 项目 | 数量 |
|---|---:|
| `DYNAMIC_ANIM` 引用 | 3330 |
| `PKGREF` 引用 | 3330 |
| 成对引用 | 3330 |

### 3.2 `skinprefabs.lua` 规模

当前 `skinprefabs.lua` 中：

| 项目 | 数量 |
|---|---:|
| `CreatePrefabSkin(...)` 条目 | 1736 |
| 带 `build_name_override` 的条目 | 379 |
| 不同 `base_prefab` | 367 |
| 不同 `build_name` | 1378 |
| 能匹配到 dynamic zip+dyn 对的条目 | 1666 |
| 有 dynamic 配对的不同 base_prefab | 364 |

### 3.3 与现有 `anim-index.json` 的覆盖关系

当前 `anim-index.json` 中的 prefab 名能直接匹配到 `skinprefabs.lua` 的 `base_prefab` 的条目约为：

```text
1052 / 1736
```

未匹配的例子主要是玩家角色类 prefab，例如：

```text
wilson
wathgrithr
winona
wendy
...
```

原因是这些角色 prefab 不是通过直接 `Prefab("wilson", ...)` 定义的，而是通过类似：

```lua
return MakePlayerCharacter("wilson", ...)
```

的工厂函数生成。现有 `anim-index` 暂未解析这类工厂函数。

但这不影响皮肤映射本身；后续可以把 `skinprefabs.lua` 单独建一个 skin-index，而不是强行依赖当前 prefab 索引。

### 3.4 `dst-anim-tool` 渲染验证

`dst-anim-tool` 已经支持皮肤渲染：

```bash
dst-anim-tool render \
  -i anim/ghost_abigail_build.zip \
  -i anim/ghost_abigail.zip \
  --skin anim/dynamic/abigail_ice.dyn \
  -- ghost/idle_custom output_dir
```

验证结果：

- 能正常输出 PNG 帧序列；
- 带皮肤与不带皮肤的输出帧哈希不同，说明皮肤 build / atlas 确实参与渲染。

另一组验证：

```bash
dst-anim-tool render \
  -i anim/wilson.zip \
  -i anim/player_basic.zip \
  --skin anim/dynamic/wilson_ice.dyn \
  -- wilson/run_loop_side output_dir
```

验证结果：

- 成功输出 16 帧；
- 与不带皮肤的输出帧哈希不同；
- 说明玩家角色皮肤同样可以通过 `base build + player anim + skin build/dyn` 组合渲染。

## 4. 建议的数据模型

后续可以新增一个独立的 skin 索引文件，例如：

```json
{
  "prefab_skins": {
    "abigail": [
      {
        "skin": "abigail_ice",
        "build": "abigail_ice",
        "zip": "dynamic/abigail_ice.zip",
        "dyn": "dynamic/abigail_ice.dyn"
      }
    ],
    "abigail_flower": [
      {
        "skin": "abigail_flower_ice",
        "build": "abigail_ice",
        "zip": "dynamic/abigail_ice.zip",
        "dyn": "dynamic/abigail_ice.dyn"
      }
    ]
  }
}
```

要点：

- 以 `base_prefab` 作为一级 key；
- `build` 优先取 `build_name_override`，否则用 skin 名；
- `zip` / `dyn` 根据 build 名在 `dynamic/` 目录中查找；
- 同一个 build 可以被多个 prefab skin 共享。

## 5. 建议的前端交互

### 5.1 Prefab 搜索页

搜索项仍保持精简：

```text
prefab 名
    动画文件数 / build 文件数
    [打开]
```

可以追加：

```text
skins: N
```

用于提示该 prefab 有多少皮肤。

### 5.2 动画详情页

在当前三栏布局中，建议在左侧栏 Animations 区域上方或内部增加一个 Skin 选择器：

```text
Skin
└── [无皮肤] [abigail_ice] [abigail_lunar] ...
```

也可以做成下拉框或紧凑列表，避免占用太多空间。

### 5.3 状态同步

选择皮肤后：

- 中间预览区重新请求渲染；
- 右侧 Symbol Dependencies 重新加载；
- 导出 GIF / PNG 序列同样带上当前皮肤；
- 切换动画时保留当前皮肤，除非该皮肤与当前 prefab 不匹配。

## 6. 建议的后端扩展

### 6.1 新 API

可以增加：

```text
GET /api/anim/assets/skins?prefab=abigail
```

返回：

```json
{
  "skins": [
    {
      "skin": "abigail_ice",
      "build": "abigail_ice",
      "zip": "dynamic/abigail_ice.zip",
      "dyn": "dynamic/abigail_ice.dyn"
    }
  ]
}
```

### 6.2 渲染参数扩展

现有渲染 API：

```text
GET /api/anim/assets/render
```

可扩展参数：

```text
skin=dynamic/abigail_ice.zip
```

或更明确：

```text
skin_zip=dynamic/abigail_ice.zip
skin_dyn=dynamic/abigail_ice.dyn
```

后端在解析 build 时，按 `.zip + .dyn` 配对加载皮肤包。

### 6.3 预览参数扩展

预览 API：

```text
GET /api/anim/assets/preview
```

同样接受 skin 参数，保证前端 PNG 帧预览与导出结果一致。

## 7. 实现阶段建议

### Phase 1：只读 skin 索引

- 解析 `prefabs/skinprefabs.lua`；
- 生成 `prefab -> skins` 映射；
- 生成 `skin -> build/zip/dyn` 映射；
- 输出 `output/skin-index.json`。

### Phase 2：WebUI 只读展示

- 搜索页展示 prefab 的 skin 数量；
- 动画详情页展示 skin 选择器；
- 暂不渲染皮肤，只显示可用皮肤列表。

### Phase 3：渲染集成

- 后端 render / preview / info 接受 skin 参数；
- 内部完成 `.zip + .dyn` 配对；
- 前端预览、导出都使用当前 skin。

### Phase 4：Symbol Dependencies 集成

- 将 skin build 的 symbols 加入候选；
- 允许用户查看 symbol 来自 base build 还是 skin build；
- 保持现有 symbol 隐藏 / 勾选逻辑一致。

## 8. 风险与开放问题

1. 玩家角色 prefab 不是直接 `Prefab(...)` 定义，当前 `anim-index` 不能直接匹配；
2. 部分 base_prefab 不是动画 prefab，只是 item skin；
3. `dynamic/*.zip` 与 `*.dyn` 的配对接近完整，但仍需处理缺文件情况；
4. 一个 build 可能被多个 skin prefab 复用；
5. 皮肤可能会覆盖部分 symbol，而不是完整替代 build；
6. 需要决定 skin 是作为“额外 build 候选”还是作为“覆盖当前 build”；
7. **symbol 匹配方式（同名 vs 间接映射）**——见 §8.1。

### 8.1 symbol 同名匹配 vs 间接映射（2026-09 调研结论）

**数据文件层面只有同名匹配。** `anim.bin` 帧元素只存 symbol 名哈希
（Klei hash，小写名）+ 帧号 + layer/z；`build.bin` 的 symbols 表存名字
与帧。引擎与 `dst-anim-tool`（render.rs `find_symbol_frame`）都按哈希
相等在当前生效 build 列表中查找，找不到即渲染为空。资源文件本身不含
任何映射表。

**间接映射只存在于引擎运行时 API，由 Lua 驱动**（脚本中
`OverrideSymbol/OverrideSkinSymbol/ClearOverrideSymbol` 共 1516 处调用）：

| API | 机制 | 实例 |
|---|---|---|
| `AnimState:SetSkin(skin_build, def_build)` | 整包替换 + 缺 symbol 回退 `def_build` | `wolfgang.lua` `SetSkin(player.gym_skin, "mighty_gym")` |
| `AnimState:OverrideSymbol(sym, build, src_sym)` | 单 symbol 跨 build 重定向，**名字可不同** | `skinner.lua` 猴子诅咒 `OverrideSymbol(sym, "wonkey", sym)` |
| `AnimState:OverrideSkinSymbol(sym, build, src_sym)` | 同上，作用于皮肤层栈，显式改名映射 | `skinner.lua` `OverrideSkinSymbol("torso_pelvis", base_skin, "torso")`（torso 填 pelvis 槽） |
| `CLOTHING[name].symbol_overrides_*` | 数据驱动的改名表，按角色/形态变化 | `skinner.lua` `src_sym = src_symbols[sym] or sym`；Wolfgang `mighty_skin` 下 `arm_upper`→`arm_upper_skin`；`symbol_overrides_by_character[prefab]` 每角色不同 |
| `AnimState:AddOverrideBuild(build)` | 追加覆盖 build，符号优先级更高 | `player_common_extensions.lua` `AddOverrideBuild("player_hit_darkness")` |
| `SetSymbolExchange(a, b)` | 交换两 symbol 的渲染层级（不改名） | `skinner.lua` 裙/衣掖边共 5 种排序 |
| `HideSymbol` / `ShowSymbol` | 隐藏/显示 | `symbol_hides`、wormwood 藏脚 hack |

**对本项目的含义：**

- 现有 Symbol Dependencies 的同名匹配是正确的：skin build（SetSkin
  路径）的设计约定即与 base build 同名、缺 symbol 回退；preview 的
  first-match + skin build 置顶已对齐该语义。
- 精确复刻“游戏内实际渲染”无法仅凭资源文件：玩家 + 衣服组合依赖
  `skinner.lua` / `clothing.lua` 的 override 表，目标 symbol → 源
  symbol 的改名映射是运行时、按角色/形态动态决定的。
- 若未来要做“角色 + 衣服”组合预览，需解析 `clothing.lua` 的
  `symbol_overrides*` 表并模拟 `skinner.lua` 的执行顺序——独立于
  skin 索引的另一条管线。

初步建议：

- 第一版把 skin 当作额外 build 候选；
- 不改变现有 symbol 选择逻辑；
- 后续如果需要更精确，再引入 `AnimState:SetSkin(build_name, def_build)` 的语义。

## 9. 后续落地入口

建议新增：

```text
src/scripts_sync/anim/skin_index.rs
```

负责：

- 解析 `prefabs/skinprefabs.lua`；
- 扫描 `data/anim/dynamic/`；
- 生成 skin 索引；
- 暴露给 WebUI。

前端建议新增：

```text
src/web/assets/app.js 中的 skin selector
```

后端建议扩展：

```text
src/web/api_data.rs 中的 anim-assets 相关 API
```

## 10. 当前不做

- 不完整实现皮肤预览；
- 不修改现有渲染接口；
- 不把 `.dyn` 直接加入现有 `anim-index.json`；
- 不重构 Symbol Dependencies 的候选模型。
