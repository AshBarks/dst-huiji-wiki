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

`scripts` 快照选择沿用项目的 `GameSource` 语义；编译结果按 snapshot 缓存在
`service::cooking::CookingDataCache`，端点：

```http
GET /api/data/cooking?snapshot=<snapshot>
```

端点只做编译，不做产物选择；候选匹配和随机都在浏览器完成。

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
  JSON 中保留 `weight` 字段，后续权重有调整时再切换为加权抽取。
- 同优先级候选以独立卡片全部显示，本次随机结果高亮；
  生命/饥饿/理智等料理数值暂后置，后续在 recipe payload 上追加 `stats` 字段即可。

## 测试

```bash
cargo test --lib cooking                 # Rust 解析/编译测试
node --test src/web/assets/js/cooking_eval.test.mjs   # 前端解释器 golden 测试
```
