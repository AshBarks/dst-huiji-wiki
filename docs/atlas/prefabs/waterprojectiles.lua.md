# `prefabs/waterprojectiles.lua`

- 扫描角色：prefabs/waterprojectiles.lua
- 归属变体（5 个）：bilesplat, inksplat, snowball, waterballoon, waterstreak_projectile
## 关联

### 组件
- `components/complexprojectile.lua`：bilesplat, inksplat, snowball, waterballoon, waterstreak_projectile（Direct；line 165）
- `components/equippable.lua`：waterballoon（Direct；line 286）
- `components/floater.lua`：waterballoon（HelperExpanded；line 256）
- `components/hauntable.lua`：waterballoon（HelperExpanded；line 295）
- `components/inspectable.lua`：waterballoon（Direct；line 280）
- `components/inventoryitem.lua`：waterballoon（Direct；line 282）
- `components/locomotor.lua`：bilesplat, inksplat, snowball, waterballoon, waterstreak_projectile（Direct；line 161）
- `components/reticule.lua`：waterballoon（Direct；line 249）
- `components/stackable.lua`：waterballoon（Direct；line 284）
- `components/watersource.lua`：waterballoon（Direct；line 291）
- `components/wateryprotection.lua`：bilesplat, inksplat, snowball, waterballoon, waterstreak_projectile（Direct；line 163）
- `components/weapon.lua`：waterballoon（Direct；line 276）

### 预制体依赖
- `bile_puddle_land`：bilesplat（Direct；line 389）
- `bile_puddle_water`：bilesplat（Direct；line 389）
- `bile_splash`：bilesplat（Direct；line 389）
- `ink_puddle_land`：inksplat（Direct；line 388）
- `ink_puddle_water`：inksplat（Direct；line 388）
- `ink_splash`：inksplat（Direct；line 388）
- `prefabs/reticule.lua`：waterballoon（Direct；line 387）
- `splash_snow_fx`：snowball（Direct；line 386）
- `waterballoon_splash`：waterballoon（Direct；line 387）
- `waterstreak_burst`：waterstreak_projectile（Direct；line 390）

### 生成引用
- `bile_puddle_land`：bilesplat（Direct；line 64）
- `bile_puddle_water`：bilesplat（Direct；line 62）
- `bile_splash`：bilesplat（Direct；line 60）
- `ink_puddle_land`：inksplat（Direct；line 83）
- `ink_puddle_water`：inksplat（Direct；line 81）
- `ink_splash`：inksplat（Direct；line 79）
- `ocean_splash_small2`：waterstreak_projectile（Direct；line 332）
- `splash_snow_fx`：snowball（Direct；line 96）
- `waterballoon_splash`：waterballoon（Direct；line 102）
- `waterstreak_burst`：waterstreak_projectile（Direct；line 329）


## 函数

### OnHitBile  [58–74]
- 归属：bilesplat

### OnHitInk  [77–93]
- 归属：inksplat

### OnHitSnow  [95–99]
- 归属：snowball

### OnHitWater  [101–105]
- 归属：waterballoon

### OnHitWaterstreak  [326–337]
- 归属：waterstreak_projectile

### ReticuleTargetFn  [222–235]
- 归属：waterballoon

### bile_fn  [360–384]
- 归属：bilesplat

### common_fn  [107–168]
- 归属：bilesplat, inksplat, snowball, waterballoon, waterstreak_projectile

### ink_fn  [300–324]
- 归属：inksplat

### onequip  [193–197]
- 归属：waterballoon

### onthrown  [204–220]
- 归属：waterballoon

### onunequip  [199–202]
- 归属：waterballoon

### onuseaswatersource  [237–243]
- 归属：waterballoon

### snowball_fn  [170–191]
- 归属：snowball

### waterballoon_fn  [245–298]
- 归属：waterballoon

### waterstreak_fn  [340–358]
- 归属：waterstreak_projectile

