# `prefabs/carnivalgame_herding.lua`

- 扫描角色：prefabs/carnivalgame_herding.lua
- 归属变体（2 个）：carnivalgame_herding_chick, carnivalgame_herding_station
## 关联

### 组件
- `components/hauntable.lua`：carnivalgame_herding_chick（HelperExpanded；line 332）
- `components/inspectable.lua`：carnivalgame_herding_chick（Direct；line 328）
- `components/knownlocations.lua`：carnivalgame_herding_chick（Direct；line 330）
- `components/locomotor.lua`：carnivalgame_herding_chick（Direct；line 294）
- `components/placer.lua`：carnivalgame_herding_chick, carnivalgame_herding_station（HelperExpanded；line 348）

### 状态图
- `stategraphs/SGcarnivalgame_herding_chick.lua`：carnivalgame_herding_chick（Direct；line 336）

### 大脑
- `brains/carnivalgame_herding_chick_brain.lua`：carnivalgame_herding_chick（Direct；line 337）

### 预制体依赖
- `carnival_confetti_fx`：carnivalgame_herding_chick（Direct；line 346）
- `carnivalgame_herding_chick`：carnivalgame_herding_station（Direct；line 345）
- `prefabs/carnival_prizeticket.lua`：carnivalgame_herding_station（Direct；line 345）
- `prefabs/carnivalgame_placementblocker.lua`：carnivalgame_herding_station（Direct；line 345）

### 生成引用
- `carnivalgame_herding_chick`：carnivalgame_herding_station（Direct；line 108）

### 行为
- `brains/carnivalgame_herding_chick_brain.lua`：RunAway, Wander（prefabs/carnivalgame_herding.lua#carnivalgame_herding_chick）


## 函数

### CreateFloorPart  [35–74]
- 归属：carnivalgame_herding_station

### CreateFlooring  [76–83]
- 归属：carnivalgame_herding_station

### OnActivateGame  [124–129]
- 归属：carnivalgame_herding_station

### OnBuilt  [85–89]
- 归属：carnivalgame_herding_station

### OnDeactivateGame  [174–189]
- 归属：carnivalgame_herding_station

### OnLaunchLanded  [282–299]
- 归属：carnivalgame_herding_chick

### OnRemoveGame  [198–200]
- 归属：carnivalgame_herding_station

### OnStartPlaying  [131–136]
- 归属：carnivalgame_herding_station

### OnStopPlaying  [142–159]
- 归属：carnivalgame_herding_station

### OnUpdateGame  [138–140]
- 归属：carnivalgame_herding_station

### RemoveGameItems  [191–196]
- 归属：carnivalgame_herding_station

### SpawnNewChick  [91–122]
- 归属：carnivalgame_herding_station

### SpawnRewards  [166–172]
- 归属：carnivalgame_herding_station

### chick_fn  [301–343]
- 归属：carnivalgame_herding_chick

### chick_ongothome  [92–95]
- 归属：carnivalgame_herding_station

### chick_onremoved  [96–98]
- 归属：carnivalgame_herding_station

### spawnticket  [161–164]
- 归属：carnivalgame_herding_station
- 掉落锚点：162:carnival_prizeticket

### station_common_postinit  [202–217]
- 归属：carnivalgame_herding_station

### station_fn  [252–254]
- 归属：carnivalgame_herding_station

### station_master_postinit  [219–250]
- 归属：carnivalgame_herding_station

### turnoff_chick  [149–151]
- 归属：carnivalgame_herding_station

### update_chick  [276–280]
- 归属：carnivalgame_herding_chick

