# `prefabs/wobster.lua`

- 扫描角色：prefabs/wobster.lua
- 归属变体（6 个）：wobster_moonglass, wobster_moonglass_land, wobster_sheller, wobster_sheller_dead, wobster_sheller_dead_cooked, wobster_sheller_land
## 关联

### 组件
- `components/burnable.lua`：wobster_moonglass_land, wobster_sheller_land（HelperExpanded；line 395）
- `components/combat.lua`：wobster_moonglass_land, wobster_sheller_land（Direct；line 393）
- `components/complexprojectile.lua`：wobster_moonglass, wobster_sheller（Direct；line 112）
- `components/cookable.lua`：wobster_moonglass_land, wobster_sheller_dead, wobster_sheller_land（Direct；line 375,478）
- `components/edible.lua`：wobster_sheller_dead, wobster_sheller_dead_cooked（Direct；line 472,521）
- `components/floater.lua`：wobster_sheller_dead, wobster_sheller_dead_cooked（HelperExpanded；line 447,495）
- `components/freezable.lua`：wobster_moonglass_land, wobster_sheller_land（HelperExpanded；line 396）
- `components/halloweenmoonmutable.lua`：wobster_sheller_land（Direct；line 420）
- `components/hauntable.lua`：wobster_moonglass_land, wobster_sheller_land（HelperExpanded；line 398）
- `components/health.lua`：wobster_moonglass_land, wobster_sheller_land（Direct；line 387）
- `components/inspectable.lua`：wobster_moonglass_land, wobster_sheller_dead, wobster_sheller_dead_cooked, wobster_sheller_land（Direct；line 357,460,509）
- `components/inventoryitem.lua`：wobster_moonglass_land, wobster_sheller_dead, wobster_sheller_dead_cooked, wobster_sheller_land（Direct；line 360,467,516）
- `components/knownlocations.lua`：wobster_moonglass, wobster_sheller（Direct；line 213）
- `components/locomotor.lua`：wobster_moonglass, wobster_moonglass_land, wobster_sheller, wobster_sheller_land（Direct；line 200,353）
- `components/lootdropper.lua`：wobster_moonglass_land, wobster_sheller_land（Direct；line 367）
- `components/murderable.lua`：wobster_moonglass_land, wobster_sheller_land（Direct；line 365）
- `components/oceanfishable.lua`：wobster_moonglass, wobster_sheller（Direct；line 206）
- `components/perishable.lua`：wobster_moonglass_land, wobster_sheller_dead, wobster_sheller_dead_cooked, wobster_sheller_land（Direct/HelperExpanded；line 408,462,511）
- `components/sleeper.lua`：wobster_moonglass_land, wobster_sheller_land（Direct；line 383）
- `components/stackable.lua`：wobster_sheller_dead, wobster_sheller_dead_cooked（Direct；line 469,518）
- `components/tradable.lua`：wobster_moonglass_land, wobster_sheller_dead, wobster_sheller_dead_cooked, wobster_sheller_land（Direct；line 380,481,527）
- `components/weighable.lua`：wobster_moonglass, wobster_moonglass_land, wobster_sheller, wobster_sheller_land（Direct；line 197,371）

### 状态图
- `stategraphs/SGwobster.lua`：wobster_moonglass, wobster_sheller（Direct；line 215）
- `stategraphs/SGwobsterland.lua`：wobster_moonglass_land, wobster_sheller_land（Direct；line 405）

### 预制体依赖
- `moonglass_wobster_den`：wobster_moonglass_land（Direct；line 534）
- `ocean_splash_small1`：wobster_moonglass, wobster_sheller（Direct；line 535,536）
- `prefabs/moonglass.lua`：wobster_moonglass_land（Direct；line 534）
- `prefabs/wobster_den.lua`：wobster_sheller_land（Direct；line 533）
- `spoiled_fish`：wobster_sheller_dead, wobster_sheller_dead_cooked（Direct；line 537,538）
- `wobster_moonglass_land`：wobster_moonglass（Direct；line 536）
- `wobster_sheller_dead`：wobster_sheller_land（Direct；line 533）
- `wobster_sheller_dead_cooked`：wobster_sheller_dead, wobster_sheller_land（Direct；line 533,537）
- `wobster_sheller_land`：wobster_sheller（Direct；line 535）

### 生成引用
- `splash`：wobster_moonglass, wobster_moonglass_land, wobster_sheller, wobster_sheller_land（Direct；line 103,122,271）


## 函数

### SetupWeighable  [145–151]
- 归属：wobster_moonglass, wobster_moonglass_land, wobster_sheller, wobster_sheller_land

### base_land_wobster  [298–411]
- 归属：wobster_moonglass_land, wobster_sheller_land
- 掉落锚点：368:

### base_water_wobster  [153–219]
- 归属：wobster_moonglass, wobster_sheller

### enter_water  [283–296]
- 归属：wobster_moonglass_land, wobster_sheller_land

### lobster_dead_cooked_fn  [487–531]
- 归属：wobster_sheller_dead_cooked

### lobster_dead_fn  [434–485]
- 归属：wobster_sheller_dead

### moonglass_land  [426–432]
- 归属：wobster_moonglass_land

### moonglass_water  [243–247]
- 归属：wobster_moonglass

### on_dropped_as_loot  [277–281]
- 归属：wobster_moonglass_land, wobster_sheller_land

### on_ground_wobster_landed  [253–275]
- 归属：wobster_moonglass_land, wobster_sheller_land

### on_make_projectile  [111–125]
- 归属：wobster_moonglass, wobster_sheller

### on_projectile_landed  [79–109]
- 归属：wobster_moonglass, wobster_sheller

### on_reeling_in  [127–133]
- 归属：wobster_moonglass, wobster_sheller

### play_cooked_sound  [249–251]
- 归属：wobster_moonglass_land, wobster_sheller_land

### set_on_rod  [135–143]
- 归属：wobster_moonglass, wobster_sheller

### wobster_land  [413–424]
- 归属：wobster_sheller_land

### wobster_water  [230–232]
- 归属：wobster_sheller

