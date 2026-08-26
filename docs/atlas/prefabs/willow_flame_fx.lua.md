# `prefabs/willow_flame_fx.lua`

- 扫描角色：prefabs/willow_flame_fx.lua
- 归属变体（3 个）：willow_frenzy, willow_shadow_flame, willow_throw_flame
## 关联

### 组件
- `components/damagetypebonus.lua`：willow_shadow_flame（Direct；line 221）
- `components/firefx.lua`：willow_shadow_flame, willow_throw_flame（Direct；line 209,262）
- `components/planardamage.lua`：willow_shadow_flame（Direct；line 217）
- `components/updatelooper.lua`：willow_frenzy（Direct；line 314）
- `components/weapon.lua`：willow_shadow_flame（Direct；line 214）

### 预制体依赖
- `prefabs/firefx_light.lua`：willow_shadow_flame, willow_throw_flame（Direct；line 375,376）
- `willow_shadow_fire_explode`：willow_shadow_flame, willow_throw_flame（Direct；line 375,376）

### 生成引用
- `deerclops_laserscorch`：willow_throw_flame（Direct；line 268）
- `willow_shadow_fire_explode`：willow_shadow_flame（Direct；line 125）
- `willow_shadow_flame`：willow_shadow_flame（Direct；line 172）


## 函数

### AddFrenzyFX  [291–318]
- 归属：willow_frenzy

### FrenzyDoOnClientInit  [320–324]
- 归属：willow_frenzy

### FrenzyKill  [336–339]
- 归属：willow_frenzy

### FrenzyOnUpdate  [282–287]
- 归属：willow_frenzy

### OnFrenzyKilled  [326–334]
- 归属：willow_frenzy

### TargetIsHostile  [42–50]
- 归属：willow_shadow_flame

### frenzyfn  [341–373]
- 归属：willow_frenzy

### settarget  [52–180]
- 归属：willow_shadow_flame

### shadowfn  [182–234]
- 归属：willow_shadow_flame

### throwfn  [236–278]
- 归属：willow_throw_flame

