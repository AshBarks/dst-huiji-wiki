# `prefabs/cave_entrance.lua`

- 扫描角色：prefabs/cave_entrance.lua
- 归属变体（3 个）：cave_entrance, cave_entrance_open, cave_entrance_ruins
## 关联

### 组件
- `components/childspawner.lua`：cave_entrance_open（Direct；line 210）
- `components/inspectable.lua`：cave_entrance, cave_entrance_open, cave_entrance_ruins（Direct；line 134）
- `components/lootdropper.lua`：cave_entrance, cave_entrance_ruins（Direct；line 172,193）
- `components/pointofinterest.lua`：cave_entrance（Direct；line 144）
- `components/workable.lua`：cave_entrance, cave_entrance_ruins（Direct；line 163,185）
- `components/worldmigrator.lua`：cave_entrance, cave_entrance_open, cave_entrance_ruins（Direct；line 135）

### 预制体依赖
- `prefabs/bat.lua`：cave_entrance, cave_entrance_open, cave_entrance_ruins（Direct；line 243,244,245）
- `prefabs/rock_break_fx.lua`：cave_entrance, cave_entrance_open, cave_entrance_ruins（Direct；line 243,244,245）


## 函数

### GetStatus  [81–85]
- 归属：cave_entrance_open

### OnIsDay  [45–54]
- 归属：cave_entrance_open

### OnPreLoad  [199–201]
- 归属：cave_entrance_open

### OnWork  [56–79]
- 归属：cave_entrance, cave_entrance_ruins

### ReturnChildren  [36–43]
- 归属：cave_entrance_open

### activatebyother  [91–93]
- 归属：cave_entrance, cave_entrance_ruins

### canspawn  [87–89]
- 归属：cave_entrance_open

### close  [20–22]
- 归属：cave_entrance_open

### closed_fn  [149–176]
- 归属：cave_entrance
- 掉落锚点：173:rocks+rocks+flint+flint+flint

### closed_init_poi  [140–147]
- 归属：cave_entrance

### fn  [95–138]
- 归属：cave_entrance, cave_entrance_open, cave_entrance_ruins

### full  [28–30]
- 归属：cave_entrance_open

### open  [24–26]
- 归属：cave_entrance_open

### open_fn  [203–241]
- 归属：cave_entrance_open

### ruins_fn  [178–197]
- 归属：cave_entrance_ruins
- 掉落锚点：194:thulecite+thulecite_pieces+thulecite_pieces

