# `prefabs/spiderhole.lua`

- 扫描角色：prefabs/spiderhole.lua
- 归属变体（2 个）：spiderhole, spiderhole_rock
## 关联

### 组件
- `components/childspawner.lua`：spiderhole（Direct；line 223）
- `components/hauntable.lua`：spiderhole, spiderhole_rock（HelperExpanded；line 246,272）
- `components/health.lua`：spiderhole（Direct；line 216）
- `components/inspectable.lua`：spiderhole, spiderhole_rock（Direct；line 172）
- `components/lootdropper.lua`：spiderhole_rock（Direct；line 269）
- `components/workable.lua`：spiderhole, spiderhole_rock（Direct；line 173）

### 预制体依赖
- `--fx
    "rock_break_fx"`：spiderhole, spiderhole_rock（Direct；line 277,278）
- `--halloween
	"spooked_spider_rock_fx"`：spiderhole, spiderhole_rock（Direct；line 277,278）
- `--loot
    "rocks"`：spiderhole, spiderhole_rock（Direct；line 277,278）
- `prefabs/fossil_piece.lua`：spiderhole, spiderhole_rock（Direct；line 277,278）
- `prefabs/silk.lua`：spiderhole, spiderhole_rock（Direct；line 277,278）
- `prefabs/spidergland.lua`：spiderhole, spiderhole_rock（Direct；line 277,278）
- `spider_hider`：spiderhole, spiderhole_rock（Direct；line 277,278）
- `spider_spitter`：spiderhole, spiderhole_rock（Direct；line 277,278）

### 生成引用
- `prefabs/rock_break_fx.lua`：spiderhole_rock（Direct；line 60）
- `spiderhole_rock`：spiderhole（Direct；line 85）


## 函数

### CanTarget  [180–182]
- 归属：spiderhole

### CustomOnHaunt  [186–201]
- 归属：spiderhole

### IsInvestigator  [89–91]
- 归属：spiderhole

### OnGoHome  [127–135]
- 归属：spiderhole

### OnPreLoad  [203–205]
- 归属：spiderhole

### OnQuakeBegin  [38–47]
- 归属：spiderhole

### OnQuakeEnd  [49–55]
- 归属：spiderhole

### SpawnInvestigators  [93–107]
- 归属：spiderhole

### SummonChildren  [109–119]
- 归属：spiderhole, spiderhole_rock

### commonfn  [137–178]
- 归属：spiderhole, spiderhole_rock

### rock_onworked  [57–75]
- 归属：spiderhole_rock

### rockfn  [254–275]
- 归属：spiderhole_rock

### spawner_onfinish  [77–87]
- 归属：spiderhole

### spawner_onworked  [121–125]
- 归属：spiderhole

### spawnerfn  [207–252]
- 归属：spiderhole

