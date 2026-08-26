# `prefabs/carnivalgame_feedchicks.lua`

- 扫描角色：prefabs/carnivalgame_feedchicks.lua
- 归属变体（3 个）：carnivalgame_feedchicks_food, carnivalgame_feedchicks_nest, carnivalgame_feedchicks_station
## 关联

### 组件
- `components/carnivalgamefeedable.lua`：carnivalgame_feedchicks_nest（Direct；line 386）
- `components/carnivalgameitem.lua`：carnivalgame_feedchicks_food（Direct；line 446）
- `components/equippable.lua`：carnivalgame_feedchicks_food（Direct；line 450）
- `components/floater.lua`：carnivalgame_feedchicks_food（HelperExpanded；line 430）
- `components/hauntable.lua`：carnivalgame_feedchicks_food（HelperExpanded；line 454）
- `components/inspectable.lua`：carnivalgame_feedchicks_food, carnivalgame_feedchicks_nest, carnivalgame_feedchicks_station（Direct；line 222,384,444）
- `components/inventoryitem.lua`：carnivalgame_feedchicks_food（Direct；line 448）
- `components/objectspawner.lua`：carnivalgame_feedchicks_station（Direct；line 271）
- `components/placer.lua`：carnivalgame_feedchicks_food, carnivalgame_feedchicks_nest, carnivalgame_feedchicks_station（HelperExpanded；line 463）

### 状态图
- `stategraphs/SGcarnivalgame_feedchicks_nest.lua`：carnivalgame_feedchicks_nest（Direct；line 391）

### 预制体依赖
- `carnivalgame_feedchicks_food`：carnivalgame_feedchicks_station（Direct；line 461）
- `carnivalgame_feedchicks_nest`：carnivalgame_feedchicks_station（Direct；line 461）
- `prefabs/carnival_prizeticket.lua`：carnivalgame_feedchicks_station（Direct；line 461）


## 函数

### CreateFloor  [52–81]
- 归属：carnivalgame_feedchicks_station

### DoActivateRandomNest  [102–114]
- 归属：carnivalgame_feedchicks_station

### NewObject  [231–241]
- 归属：carnivalgame_feedchicks_station

### OnActivateGame  [120–142]
- 归属：carnivalgame_feedchicks_station

### OnBuilt  [83–99]
- 归属：carnivalgame_feedchicks_station

### OnDeactivateGame  [210–229]
- 归属：carnivalgame_feedchicks_station

### OnRemoveGame  [243–247]
- 归属：carnivalgame_feedchicks_station

### OnStartPlaying  [144–148]
- 归属：carnivalgame_feedchicks_station

### OnStopPlaying  [175–195]
- 归属：carnivalgame_feedchicks_station

### OnUpdateGame  [150–157]
- 归属：carnivalgame_feedchicks_station

### RemoveGameItems  [159–173]
- 归属：carnivalgame_feedchicks_station

### SpawnRewards  [202–208]
- 归属：carnivalgame_feedchicks_station

### activate_nest  [103–105]
- 归属：carnivalgame_feedchicks_station

### create_nest_points  [30–49]
- 归属：（未归属）

### createplacernest  [315–333]
- 归属：（未归属）

### displaynamefn_nest  [351–353]
- 归属：carnivalgame_feedchicks_nest

### food_onequip  [396–406]
- 归属：carnivalgame_feedchicks_food

### food_onunequip  [408–415]
- 归属：carnivalgame_feedchicks_food

### fooditem_fn  [417–457]
- 归属：carnivalgame_feedchicks_food

### nest_turnon  [116–118]
- 归属：carnivalgame_feedchicks_station

### nestfn  [355–394]
- 归属：carnivalgame_feedchicks_nest

### onfeed  [232–234]
- 归属：carnivalgame_feedchicks_station

### onfeed_nest  [346–349]
- 归属：carnivalgame_feedchicks_nest

### onnestavailable  [235–238]
- 归属：carnivalgame_feedchicks_station

### placerdecor  [335–344]
- 归属：（未归属）

### spawnticket  [197–200]
- 归属：carnivalgame_feedchicks_station
- 掉落锚点：198:carnival_prizeticket

### station_common_postinit  [249–259]
- 归属：carnivalgame_feedchicks_station

### station_fn  [292–294]
- 归属：carnivalgame_feedchicks_station

### station_master_postinit  [261–290]
- 归属：carnivalgame_feedchicks_station

### turnoff_nest  [184–186]
- 归属：carnivalgame_feedchicks_station

