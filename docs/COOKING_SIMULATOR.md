# 烹饪模拟器

WebUI 的“烹饪模拟”页：玩家选择 4 个可烹饪食材和锅类型，页面列出所有满足条件的食谱；
若最高优先级候选不止一个，则等概率随机一个作为本次产出，并同时展示全部同优先级可能。

## 数据来源

- 食谱：`scripts/preparedfoods.lua`、`scripts/preparednonfoods.lua`、`scripts/preparedfoods_warly.lua`
- cooker 归属：
  - `cookpot`：`preparedfoods` + `preparednonfoods`
  - `portablecookpot`：`preparedfoods` + `preparednonfoods` + `preparedfoods_warly`
- 食材属性：`scripts/cooking.lua` 的 `AddIngredientValues` 调用
  - 包含 `_cooked` / `_dried` 隐式变体与 `aliases`
  - 海鱼食材由 `scripts/prefabs/oceanfishdef.lua` 的 `FISH_DEFS[*].cooker_ingredient_value` 展开为 `<prefab>_inv`
- 真实 prefab 白名单：`scripts/scrapbook_prefabs.lua` 与显式例外
  `fish` / `fish_cooked`（`prefabs/fish.lua` 仍会注册它们，scrapbook 中将其注释为 deprecated）
- 料理食物类型：`preparedfoods*.lua` 每条食谱的 `foodtype` / `secondaryfoodtype`（`FOODTYPE.*` 常量）
  - `preparedfoods.lua` / `preparedfoods_warly.lua` 省略 `foodtype` 时按 `prefabs/preparedfoods.lua`
    的 `data.foodtype or FOODTYPE.GENERIC` 落 `GENERIC`（当前只有 `wetgoop` 命中）
  - `preparednonfoods.lua` 不设默认：条目不会隐式变成 prefab，`batnosehat` 是帽子、本就不是食物，
    因此不带食物类型；非 `FOODTYPE.<NAME>` 常量一律报错，避免静默错标
  - 中文名取自 PO 的 `STRINGS.SCRAPBOOK.FOODTYPE.*`（`肉` / `好东西` / `通用` …），PO 未收录的
    `NITRE` / `MIASMA` 由 `cooking_assets::FOODTYPE_LABEL_FALLBACK` 兜底
  - 该字段同时进入 WebUI payload 与 `cooking-game-export` 数据包（`foodtype` / `foodtype_zh` /
    `secondary_foodtype` / `secondary_foodtype_zh`，bundle schema 升到 3）；食物类型是食物自身的
    属性，与食材的「食材属性度」（`tags`）无关，不能用 tags 反推

`scripts` 快照选择沿用项目的 `GameSource` 语义；编译结果按 snapshot 缓存在
`service::cooking::CookingDataCache`，端点：

```http
GET /api/data/cooking?snapshot=<snapshot>
```

端点只做编译，不做产物选择；候选匹配和随机都在浏览器完成。

## 料理数值（生命 / 饥饿 / 理智 / 温度）

每条食谱的 `health` / `hunger` / `sanity`（以及冷热食物的 `temperature` +
`temperatureduration`）是**入口瞬时值**，取自 `preparedfoods*.lua` 的条目字段；
落到 prefab 时由 `prefabs/preparedfoods.lua` 写进 `edible.healthvalue` /
`hungervalue` / `sanityvalue`（只有 `sanity` 的缺省是 0）。

- 数值是 `TUNING.*` 常量与 `+ - * /`、一元负号、括号的组合（如
  `TUNING.CALORIES_LARGE + TUNING.CALORIES_MEDSMALL`）。`src/parser/cooking/tuning.rs`
  从 `tuning.lua` 的默认 `Tune()`（文件末尾无参调用）重建这些常量：先收集 `Tune()`
  内的常量 local（`calories_per_day = 75`、`day_time = seg_time * day_segs` 等），
  再惰性求值 `TUNING = { ... }` 表条目。
- **白名单**：食谱只能引用 `ALLOWED_CONSTANTS`（30 个）里的常量；表外常量、表内不存在的常量、
  裸标识符、函数调用、多级点路径等形态一律解析报错——游戏更新引入新数值时必须显式补表，
  而不是静默算出一个错的数值。
- `preparednonfoods.lua` 的条目（`batnosehat`、`dustmeringue`）**一律导出为 0/0/0**：
  前者本体是帽子（`hats.lua`），后者的真实数值在自己的 `prefabs/dustmeringue.lua`
  （只设 `hungervalue = TUNING.CALORIES_SMALL`，health/sanity 走 `edible` 默认 10/0），
  条目里的数字只是图鉴口径。见 `recipes::StatsPolicy::Zero`。
- 口径是「如实记录入口值」：`jellybean` 记 `health = 2`（另有持续回血 buff，图鉴显示 122）、
  `sweettea` 记 `sanity = 15`（图鉴显示 45），不做图鉴值替换、不加特殊效果注释。
- **不在本数据范围内**：持续 buff / 特殊效果文案、角色倍率（沃姆伍德生命吸收 0、
  沃拓克斯 ×0.5、沃利重复料理惩罚 `0.9/0.8/0.65/0.5/0.3`）、食物腐败后的衰减、
  香料变体（`spicedfoods.lua`，本就不是锅的候选产物）。
- 温度含 `no_chill`（`nochill`，当前仅 `dragonchilisalad` 一条）。

## 白名单一次性交叉验证

开发阶段对线上 wiki 的 `Data:DST Prefab/*.json` 页面做了一次
`cooking_ingredient == true` 过滤，与本地游戏脚本计算结果对账：

- wiki 数据页（namespace 3500，前缀 `DST Prefab/`）共 3869 页；
- `cooking_ingredient: true` 的 prefab 共 146 个；
- 本地 `cooking.lua + oceanfishdef + scrapbook 白名单 + aliases` 也得到 146 个；
- 双向 diff 为空。

验证时还发现 `fish` / `fish_cooked` 虽被 `scrapbook_prefabs.lua` 注释为
deprecated，但 `prefabs/fish.lua` 仍注册，且 wiki 标记为 cooking ingredient，
因此它们以显式常量 `EXTRA_REAL_PREFABS` 保留在本地编译器中。

该 wiki 对账只用于本次校验，**不是运行时数据源，也不进入后续持续维护链路**；
后续脚本更新仍以本地 `cooking.lua` / `scrapbook_prefabs.lua` 为准。如果游戏更新
导致 `compiles_current_dst_data_when_configured` 的 146 / 81 断言变化，再人工复核
白名单例外即可。

## 前端语义与展示

- `cooking_eval.js` 解释 Rust 编译出的紧凑 AST，实现 Lua 的：
  - 只有 `nil` / `false` 为假（数值 `0` 为真）；
  - `and` / `or` 短路并返回操作数；
  - 字段缺失视为 `nil`。
- 当前所有 raw recipe `weight` 均为 1，因此前端在最高优先级集合上做等概率抽取；
  JSON 中保留 `weight` 字段。若未来脚本出现非 1 weight，后端会拒绝编译，提示
  先实现加权抽取，避免静默算错。
- 同优先级候选以独立卡片全部显示，本次随机结果高亮；
  生命/饥饿/理智等料理数值已在 recipe payload 的 `stats`（以及冷热食物的
  `temperature`）字段里，WebUI 卡片展示仍后置。

## 导出数据包（cooking-game-export）

`cooking-game-export` 把同一份编译结果打包给独立的 `dst-cooking-game` 项目：

| 路径 | 内容 |
|------|------|
| `data/cooking.json` | `schema_version = 5`：食材（`tags` 属性度）、食谱（`test` AST、`desc_attrs`/`desc_special`、`foodtype`/`foodtype_zh`/`secondary_foodtype`/`secondary_foodtype_zh`、入口瞬时 `stats`（`health`/`hunger`/`sanity`）、冷热食物的 `temperature`（`delta`/`duration`/`no_chill`））、cooker 归属、每道菜的 `example_combo`；`cooking_schema_version` 是 AST 契约，字段增量为向后兼容 |
| `data/characters.json` | `schema_version = 1`：19 个可玩角色，`key` = prefab（`wathgrithr` = 薇格弗德、`waxwell` = 麦斯威尔），`name_en`/`name_zh` 取自 `STRINGS.CHARACTER_NAMES.*`，`portrait` 指向包内头像 |
| `ui/characters/<key>.png` | 120×104 RGBA 存档头像，取自 `saveslot_portraits/`，与 `ui/warly.png` 同源同风格 |
| `ui/{cookpot,portablecookpot,warly}.png` | 锅具与沃利头像，路径保持不变 |
| `images/*.png` | 食材/料理的物品栏图标 |
| `manifest.json` | 每个文件的 sha256 + `counts`（含 `characters`）+ `missing_icons` |

角色清单以图片管线的 `saveslot_portraits/` 目录为准（游戏新增角色会自动带上），过滤
`background`/`mod`/`mod_small`/`random`/`unknown` 占位图；PO 里查不到名字的角色仍会导出，
只在运行日志里告警。`bundle_schema_version`（包结构）与 `game_contract_version`（游戏端契约）
分别递增，本包只动前者。

## 测试

```bash
cargo test --lib cooking                 # Rust 解析/编译测试
node --test src/web/assets/js/cooking_eval.test.mjs   # 前端解释器 golden 测试
```
