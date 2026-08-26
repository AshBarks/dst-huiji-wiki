# `prefabs/egg.lua`

- 扫描角色：prefabs/egg.lua
- 归属变体（3 个）：bird_egg, bird_egg_cooked, rottenegg
## 关联

### 组件
- `components/bait.lua`：bird_egg, bird_egg_cooked（Direct；line 71）
- `components/burnable.lua`：rottenegg（HelperExpanded；line 185）
- `components/cookable.lua`：bird_egg, bird_egg_cooked（Direct；line 57）
- `components/deployable.lua`：rottenegg（HelperExpanded；line 188）
- `components/edible.lua`：bird_egg, bird_egg_cooked, rottenegg（Direct；line 53,193）
- `components/fertilizer.lua`：rottenegg（Direct；line 169）
- `components/fertilizerresearchable.lua`：rottenegg（Direct；line 166）
- `components/floater.lua`：bird_egg, bird_egg_cooked, rottenegg（HelperExpanded；line 45,151）
- `components/fuel.lua`：rottenegg（Direct；line 183）
- `components/hauntable.lua`：bird_egg, bird_egg_cooked, rottenegg（HelperExpanded；line 66,189）
- `components/inspectable.lua`：bird_egg, bird_egg_cooked, rottenegg（Direct；line 73,175）
- `components/inventoryitem.lua`：bird_egg, bird_egg_cooked, rottenegg（Direct；line 75,178）
- `components/perishable.lua`：bird_egg, bird_egg_cooked（Direct；line 61）
- `components/propagator.lua`：rottenegg（HelperExpanded；line 186）
- `components/stackable.lua`：bird_egg, bird_egg_cooked, rottenegg（Direct；line 68,180）
- `components/tradable.lua`：bird_egg, bird_egg_cooked, rottenegg（Direct；line 77,191）

### 预制体依赖
- `bird_egg_cooked`：bird_egg（Direct；line 201）
- `gridplacer_farmablesoil`：rottenegg（Direct；line 203）
- `rottenegg`：bird_egg（Direct；line 201）
- `spoiled_food`：bird_egg_cooked（Direct；line 202）


## 函数

### GetFertilizerKey  [121–123]
- 归属：rottenegg

### GetStatus  [129–132]
- 归属：rottenegg

### commonfn  [25–81]
- 归属：bird_egg, bird_egg_cooked

### cookedfn  [103–119]
- 归属：bird_egg_cooked

### defaultfn  [83–101]
- 归属：bird_egg

### fertilizerresearchfn  [125–127]
- 归属：rottenegg

### rottenfn  [134–199]
- 归属：rottenegg

