# `prefabs/carnivalgame_memory.lua`

- 扫描角色：prefabs/carnivalgame_memory.lua
- 归属变体（2 个）：carnivalgame_memory_card, carnivalgame_memory_station
## 关联

### 组件
- `components/activatable.lua`：carnivalgame_memory_card（Direct；line 381）
- `components/inspectable.lua`：carnivalgame_memory_card（Direct；line 378）
- `components/objectspawner.lua`：carnivalgame_memory_station（Direct；line 254）
- `components/placer.lua`：carnivalgame_memory_card, carnivalgame_memory_station（HelperExpanded；line 398）

### 状态图
- `stategraphs/SGcarnivalgame_memory_card.lua`：carnivalgame_memory_card（Direct；line 389）

### 预制体依赖
- `carnivalgame_memory_card`：carnivalgame_memory_station（Direct；line 396）
- `prefabs/carnival_prizeticket.lua`：carnivalgame_memory_station（Direct；line 396）


## 函数

### CreateFloor  [40–69]
- 归属：carnivalgame_memory_station

### DoEndOfRound  [133–143]
- 归属：carnivalgame_memory_station

### DoNextRound  [109–131]
- 归属：carnivalgame_memory_station

### GetActivateVerb  [339–341]
- 归属：carnivalgame_memory_card

### NewObject  [211–227]
- 归属：carnivalgame_memory_station

### OnActivateGame  [93–107]
- 归属：carnivalgame_memory_station

### OnBuilt  [71–87]
- 归属：carnivalgame_memory_station

### OnDeactivateGame  [195–209]
- 归属：carnivalgame_memory_station

### OnRemoveGame  [229–233]
- 归属：carnivalgame_memory_station

### OnStartPlaying  [145–150]
- 归属：carnivalgame_memory_station

### OnStopPlaying  [175–193]
- 归属：carnivalgame_memory_station

### OnUpdateGame  [152–154]
- 归属：carnivalgame_memory_station

### RemoveGameItems  [156–157]
- 归属：carnivalgame_memory_station

### SpawnRewards  [164–173]
- 归属：carnivalgame_memory_station

### card_GetStatus  [335–337]
- 归属：carnivalgame_memory_card

### card_OnPick  [330–333]
- 归属：carnivalgame_memory_card

### card_turnon  [89–91]
- 归属：carnivalgame_memory_station

### cardfn  [347–392]
- 归属：carnivalgame_memory_card

### create_card_points  [24–36]
- 归属：（未归属）

### createplacercard  [298–316]
- 归属：（未归属）

### displaynamefn_card  [343–345]
- 归属：carnivalgame_memory_card

### on_card_picked  [212–223]
- 归属：carnivalgame_memory_station

### placerdecor  [318–327]
- 归属：（未归属）

### spawnticket  [159–162]
- 归属：carnivalgame_memory_station
- 掉落锚点：160:carnival_prizeticket

### station_common_postinit  [235–245]
- 归属：carnivalgame_memory_station

### station_fn  [275–277]
- 归属：carnivalgame_memory_station

### station_master_postinit  [247–273]
- 归属：carnivalgame_memory_station

### turnoff_card  [182–184]
- 归属：carnivalgame_memory_station

