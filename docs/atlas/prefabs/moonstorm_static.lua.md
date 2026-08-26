# `prefabs/moonstorm_static.lua`

- 扫描角色：prefabs/moonstorm_static.lua
- 归属变体（5 个）：moonstorm_static, moonstorm_static_catcher, moonstorm_static_item, moonstorm_static_nowag, moonstorm_static_roamer
## 关联

### 组件
- `components/combat.lua`：moonstorm_static, moonstorm_static_nowag（Direct；line 103,217）
- `components/equippable.lua`：moonstorm_static_catcher（Direct；line 390）
- `components/floater.lua`：moonstorm_static_catcher, moonstorm_static_item（HelperExpanded；line 297,372）
- `components/health.lua`：moonstorm_static, moonstorm_static_nowag（Direct；line 99,213）
- `components/hudindicatable.lua`：moonstorm_static_roamer（Direct；line 508）
- `components/inspectable.lua`：moonstorm_static, moonstorm_static_catcher, moonstorm_static_item, moonstorm_static_nowag, moonstorm_static_roamer（Direct；line 115,229,308,385,521）
- `components/inventoryitem.lua`：moonstorm_static_catcher, moonstorm_static_item（Direct；line 309,388）
- `components/locomotor.lua`：moonstorm_static_roamer（Direct；line 523）
- `components/moonstormstaticcapturable.lua`：moonstorm_static_roamer（Direct；line 529）
- `components/moonstormstaticcatcher.lua`：moonstorm_static_catcher（Direct；line 394）
- `components/stackable.lua`：moonstorm_static_catcher（Direct；line 381）
- `components/tradable.lua`：moonstorm_static_item（Direct；line 307）
- `components/trader.lua`：moonstorm_static_nowag（Direct；line 232）
- `components/upgrader.lua`：moonstorm_static_item（Direct；line 311）

### 状态图
- `stategraphs/SGmoonstormstatic.lua`：moonstorm_static_roamer（Direct；line 532）

### 大脑
- `brains/moonstormstaticbrain.lua`：moonstorm_static_roamer（Direct；line 533）

### 预制体依赖
- `moonstorm_static_item`：moonstorm_static, moonstorm_static_nowag（Direct；line 546,547）
- `moonstorm_static_nowag`：moonstorm_static_catcher（Direct；line 549）

### 行为
- `brains/moonstormstaticbrain.lua`：Wander（prefabs/moonstorm_static.lua#moonstorm_static_roamer）


## 函数

### Decay  [409–416]
- 归属：moonstorm_static_roamer

### MakePlayerMade  [264–269]
- 归属：moonstorm_static_item

### OnCaught_catcher  [351–353]
- 归属：moonstorm_static_catcher

### OnCaught_roamer  [464–470]
- 归属：moonstorm_static_roamer

### OnEntitySleep  [260–262]
- 归属：moonstorm_static_item

### OnEntityWake  [250–258]
- 归属：moonstorm_static_item

### OnEquip_catcher  [340–344]
- 归属：moonstorm_static_catcher

### OnLoad_item  [273–279]
- 归属：moonstorm_static_item

### OnSave_item  [270–272]
- 归属：moonstorm_static_item

### OnUnequip_catcher  [346–349]
- 归属：moonstorm_static_catcher

### OnZigZagUpdate  [437–462]
- 归属：moonstorm_static_roamer

### PlayInitAnimation  [164–168]
- 归属：moonstorm_static_nowag

### PlayInitAnimation_pst  [159–163]
- 归属：moonstorm_static_nowag

### ShouldTrackfn_roamer  [473–479]
- 归属：moonstorm_static_roamer

### StartDecay  [418–427]
- 归属：moonstorm_static_roamer

### StopDecay  [429–435]
- 归属：moonstorm_static_roamer

### finished  [41–47]
- 归属：moonstorm_static, moonstorm_static_nowag

### finished_callback  [34–40]
- 归属：moonstorm_static, moonstorm_static_nowag

### fn  [58–118]
- 归属：moonstorm_static

### fn_catcher  [355–398]
- 归属：moonstorm_static_catcher

### fn_roamer  [481–544]
- 归属：moonstorm_static_roamer

### itemfn  [281–325]
- 归属：moonstorm_static_item

### nowag_fn  [170–245]
- 归属：moonstorm_static_nowag

### on_get_item_from_player  [144–148]
- 归属：moonstorm_static_nowag

### on_nowag_need_tool  [150–153]
- 归属：moonstorm_static_nowag

### on_nowag_need_tool_over  [154–157]
- 归属：moonstorm_static_nowag

### on_refuse_item  [138–142]
- 归属：moonstorm_static_nowag

### onattackedfn  [16–22]
- 归属：moonstorm_static, moonstorm_static_nowag

### ondeath  [24–32]
- 归属：moonstorm_static, moonstorm_static_nowag

### should_accept_item  [125–136]
- 归属：moonstorm_static_nowag

### stormstopped  [54–56]
- 归属：moonstorm_static, moonstorm_static_nowag

### stormstopped_callback  [49–53]
- 归属：moonstorm_static, moonstorm_static_nowag

