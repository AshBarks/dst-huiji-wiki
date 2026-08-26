# `prefabs/lunar_grazer.lua`

- 扫描角色：prefabs/lunar_grazer.lua
- 归属变体（3 个）：lunar_grazer, lunar_grazer_core_fx, lunar_grazer_debris
## 关联

### 组件
- `components/combat.lua`：lunar_grazer（Direct；line 439）
- `components/damagetyperesist.lua`：lunar_grazer（Direct；line 452）
- `components/entitytracker.lua`：lunar_grazer（Direct；line 462）
- `components/gestaltcapturable.lua`：lunar_grazer（Direct；line 464）
- `components/health.lua`：lunar_grazer（Direct；line 434）
- `components/inspectable.lua`：lunar_grazer（Direct；line 432）
- `components/knownlocations.lua`：lunar_grazer（Direct；line 461）
- `components/locomotor.lua`：lunar_grazer（Direct；line 455）
- `components/planardamage.lua`：lunar_grazer（Direct；line 449）
- `components/planarentity.lua`：lunar_grazer（Direct；line 448）

### 状态图
- `stategraphs/SGlunar_grazer.lua`：lunar_grazer（Direct；line 512）

### 大脑
- `brains/lunar_grazer_brain.lua`：lunar_grazer（Direct；line 513）

### 预制体依赖
- `lunar_grazer_core_fx`：lunar_grazer（Direct；line 586）
- `lunar_grazer_debris`：lunar_grazer（Direct；line 586）
- `prefabs/lunar_goop_cloud_fx.lua`：lunar_grazer（Direct；line 586）
- `prefabs/lunar_goop_trail_fx.lua`：lunar_grazer（Direct；line 586）

### 行为
- `brains/lunar_grazer_brain.lua`：ChaseAndAttack, Leash, Wander（prefabs/lunar_grazer.lua#lunar_grazer）


## 函数

### DoCloudTask  [192–216]
- 归属：lunar_grazer

### DropDebris  [138–155]
- 归属：lunar_grazer

### EnableCloud  [231–241]
- 归属：lunar_grazer

### HideDebris  [75–85]
- 归属：lunar_grazer

### IsCloudEnabled  [243–245]
- 归属：lunar_grazer

### IsTargetSleeping  [249–256]
- 归属：lunar_grazer

### KeepTargetFn  [306–315]
- 归属：lunar_grazer

### OnAttacked  [317–326]
- 归属：lunar_grazer

### OnCaptured  [328–331]
- 归属：lunar_grazer

### OnClearCloudProtection  [179–181]
- 归属：lunar_grazer

### OnEntitySleep  [335–342]
- 归属：lunar_grazer

### OnEntityWake  [344–349]
- 归属：lunar_grazer

### OnLoad  [364–368]
- 归属：lunar_grazer

### OnLoadPostPass  [370–377]
- 归属：lunar_grazer

### OnNewState  [166–170]
- 归属：lunar_grazer

### OnRemoveEntity  [157–164]
- 归属：lunar_grazer

### OnSave  [358–362]
- 归属：lunar_grazer

### OnSpawnedBy  [351–356]
- 归属：lunar_grazer

### RecycleTrail  [22–32]
- 归属：lunar_grazer

### RetargetFn  [258–304]
- 归属：lunar_grazer

### ScatterDebris  [104–117]
- 归属：lunar_grazer

### SetCloudProtection  [183–190]
- 归属：lunar_grazer

### ShowDebris  [87–102]
- 归属：lunar_grazer

### SpawnTrail  [34–55]
- 归属：lunar_grazer

### StartCloudTask  [218–222]
- 归属：lunar_grazer

### StartTracking  [57–59]
- 归属：lunar_grazer

### StopCloudTask  [224–229]
- 归属：lunar_grazer

### StopTracking  [61–69]
- 归属：lunar_grazer

### TossDebris  [119–136]
- 归属：lunar_grazer

### corefn  [520–545]
- 归属：lunar_grazer_core_fx

### debrisfn  [549–582]
- 归属：lunar_grazer_debris

### fn  [381–516]
- 归属：lunar_grazer

