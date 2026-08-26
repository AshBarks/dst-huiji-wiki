# `prefabs/pond.lua`

- 扫描角色：prefabs/pond.lua
- 归属变体（3 个）：pond, pond_cave, pond_mos
## 关联

### 组件
- `components/acidlevel.lua`：pond_cave（Direct；line 492）
- `components/childspawner.lua`：pond, pond_cave, pond_mos（Direct；line 252）
- `components/fishable.lua`：pond, pond_cave, pond_mos（Direct；line 264）
- `components/hauntable.lua`：pond, pond_cave, pond_mos（Direct；line 267）
- `components/inspectable.lua`：pond, pond_cave, pond_mos（Direct；line 261）
- `components/lootdropper.lua`：pond_cave（Direct；line 482）
- `components/slipperyfeettarget.lua`：pond, pond_mos（Direct；line 163）
- `components/watersource.lua`：pond, pond_cave, pond_mos（Direct；line 270）
- `components/workable.lua`：pond_cave（Direct；line 484）

### 预制体依赖
- `pondeel`：pond_cave（Direct；line 510）
- `prefabs/frog.lua`：pond（Direct；line 508）
- `prefabs/marsh_plant.lua`：pond, pond_mos（Direct；line 508,509）
- `prefabs/mosquito.lua`：pond_mos（Direct；line 509）
- `prefabs/nitre.lua`：pond_cave（Direct；line 510）
- `prefabs/nitre_formation.lua`：pond_cave（Direct；line 510）
- `prefabs/pondfish.lua`：pond, pond_mos（Direct；line 508,509）


## 函数

### DespawnNitreFormations  [69–82]
- 归属：pond_cave

### DespawnPlants  [122–134]
- 归属：pond, pond_mos

### OnAcidLevelDelta_Cave  [414–447]
- 归属：pond_cave

### OnInit  [296–302]
- 归属：pond, pond_mos

### OnIsDay  [287–294]
- 归属：pond, pond_mos

### OnLoad  [194–200]
- 归属：pond, pond_cave, pond_mos

### OnPondCaveMinedFinished  [456–463]
- 归属：pond_cave
- 掉落锚点：459:nitre

### OnPreLoadCave  [210–212]
- 归属：pond_cave

### OnPreLoadFrog  [206–208]
- 归属：pond

### OnPreLoadMosquito  [202–204]
- 归属：pond_mos

### OnSave  [189–192]
- 归属：pond, pond_cave, pond_mos

### OnSnowLevel  [145–187]
- 归属：pond, pond_mos

### OnStopIsAcidRaining  [449–454]
- 归属：pond_cave

### PlayBubble  [374–379]
- 归属：pond_cave

### PondCaveDisplayNameFn  [465–467]
- 归属：pond_cave

### ReturnChildren  [278–285]
- 归属：pond, pond_mos

### SetAcidic_Cave  [398–412]
- 归属：pond_cave

### SetBackToNormal_Cave  [381–396]
- 归属：pond_cave

### SlipperyRate  [136–143]
- 归属：pond, pond_mos

### SpawnNitreFormations  [30–67]
- 归属：pond_cave

### SpawnPlants  [84–120]
- 归属：pond, pond_cave, pond_mos

### commonfn  [214–276]
- 归属：pond, pond_cave, pond_mos

### pondcave  [469–505]
- 归属：pond_cave

### pondfrog  [340–372]
- 归属：pond

### pondmos  [304–338]
- 归属：pond_mos

