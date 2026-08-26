# `prefabs/spat.lua`

- 扫描角色：prefabs/spat.lua
- 归属变体（2 个）：spat, spat_bomb
## 关联

### 组件
- `components/burnable.lua`：spat（HelperExpanded；line 264）
- `components/combat.lua`：spat（Direct；line 238）
- `components/complexprojectile.lua`：spat_bomb（Direct；line 389）
- `components/eater.lua`：spat（Direct；line 235）
- `components/equippable.lua`：spat（Direct；line 103,120）
- `components/freezable.lua`：spat（HelperExpanded；line 265）
- `components/hauntable.lua`：spat（HelperExpanded；line 274）
- `components/health.lua`：spat（Direct；line 245）
- `components/inspectable.lua`：spat（Direct；line 253）
- `components/inventory.lua`：spat（Direct；line 248）
- `components/inventoryitem.lua`：spat（Direct；line 100,117）
- `components/locomotor.lua`：spat, spat_bomb（Direct；line 267,388）
- `components/lootdropper.lua`：spat（Direct；line 250）
- `components/periodicspawner.lua`：spat（Direct；line 257）
- `components/prophider.lua`：spat（Direct；line 283）
- `components/sleeper.lua`：spat（Direct；line 271）
- `components/weapon.lua`：spat（Direct；line 96,114）

### 状态图
- `stategraphs/SGspat.lua`：spat（Direct；line 278）

### 大脑
- `brains/spatbrain.lua`：spat（Direct；line 277）

### 预制体依赖
- `meat`：spat（Direct；line 398）
- `prefabs/phlegm.lua`：spat（Direct；line 398）
- `prefabs/poop.lua`：spat（Direct；line 398）
- `prefabs/steelwool.lua`：spat（Direct；line 398）
- `spat_bomb`：spat（Direct；line 398）
- `spat_splash_fx_full`：spat_bomb（Direct；line 399）
- `spat_splash_fx_low`：spat_bomb（Direct；line 399）
- `spat_splash_fx_med`：spat_bomb（Direct；line 399）
- `spat_splash_fx_melted`：spat_bomb（Direct；line 399）
- `spat_splat_fx`：spat_bomb（Direct；line 399）

### 生成引用
- `koalefantcorpse_prop`：spat（Direct；line 171）
- `spat_splat_fx`：spat_bomb（Direct；line 298）

### 行为
- `brains/spatbrain.lua`：ChaseAndAttack, FaceEntity, RunAway, Wander（prefabs/spat.lua#spat）


## 函数

### CustomOnHaunt  [129–132]
- 归属：spat

### EquipWeapons  [90–127]
- 归属：spat

### KeepTarget  [76–78]
- 归属：spat

### OnAttacked  [80–88]
- 归属：spat

### OnForceSleep  [134–138]
- 归属：spat

### OnHideFn  [164–168]
- 归属：spat

### OnProjectileHit  [325–328]
- 归属：spat_bomb

### OnSpawnedForHunt  [180–201]
- 归属：spat

### OnUnhideFn  [153–162]
- 归属：spat

### OnVisibleFn  [140–142]
- 归属：spat

### PropCreationFn  [170–178]
- 归属：spat

### Retarget  [63–74]
- 归属：spat

### WillUnhideFn  [144–151]
- 归属：spat

### doprojectilehit  [295–323]
- 归属：spat_bomb

### fn  [203–293]
- 归属：spat

### oncollide  [330–348]
- 归属：spat_bomb

### projectilefn  [350–396]
- 归属：spat_bomb

