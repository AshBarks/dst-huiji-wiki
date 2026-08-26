# `prefabs/shadowthrall_centipede.lua`

- 扫描角色：prefabs/shadowthrall_centipede.lua
- 归属变体（4 个）：shadowthrall_centipede_body, shadowthrall_centipede_controller, shadowthrall_centipede_head, shadowthrall_centipede_spawner
## 关联

### 组件
- `components/centipedebody.lua`：shadowthrall_centipede_controller（Direct；line 397）
- `components/combat.lua`：shadowthrall_centipede_body, shadowthrall_centipede_controller, shadowthrall_centipede_head（Direct；line 243,387）
- `components/drownable.lua`：shadowthrall_centipede_body, shadowthrall_centipede_head（Direct；line 274）
- `components/eater.lua`：shadowthrall_centipede_body, shadowthrall_centipede_head（Direct；line 253）
- `components/hauntable.lua`：shadowthrall_centipede_body, shadowthrall_centipede_head（HelperExpanded；line 293）
- `components/health.lua`：shadowthrall_centipede_body, shadowthrall_centipede_controller, shadowthrall_centipede_head（Direct；line 240,383）
- `components/inspectable.lua`：shadowthrall_centipede_body, shadowthrall_centipede_head（Direct；line 237）
- `components/knownlocations.lua`：shadowthrall_centipede_controller（Direct；line 381）
- `components/locomotor.lua`：shadowthrall_centipede_body, shadowthrall_centipede_head（Direct；line 264）
- `components/lootdropper.lua`：shadowthrall_centipede_body, shadowthrall_centipede_head（Direct；line 256）
- `components/planardamage.lua`：shadowthrall_centipede_body, shadowthrall_centipede_head（Direct；line 261）
- `components/planarentity.lua`：shadowthrall_centipede_body, shadowthrall_centipede_controller, shadowthrall_centipede_head（Direct；line 259,394）
- `components/sanityaura.lua`：shadowthrall_centipede_body, shadowthrall_centipede_head（Direct；line 271）
- `components/teleportedoverride.lua`：shadowthrall_centipede_body, shadowthrall_centipede_head（Direct；line 268）
- `components/timer.lua`：shadowthrall_centipede_body, shadowthrall_centipede_head（Direct；line 275）

### 状态图
- `stategraphs/SGshadowthrall_centipede.lua`：shadowthrall_centipede_body, shadowthrall_centipede_head（Direct；line 277）

### 大脑
- `brains/shadowthrall_centipede_brain.lua`：shadowthrall_centipede_head（Direct；line 320）
- `brains/shadowthrall_centipede_controller_brain.lua`：shadowthrall_centipede_controller（Direct；line 401）

### 预制体依赖
- `--#DELETEME
    "shadowthrall_centipede_spawner"`：shadowthrall_centipede_controller（Direct；line 485）
- `shadowthrall_centipede_body`：shadowthrall_centipede_controller（Direct；line 485）
- `shadowthrall_centipede_head`：shadowthrall_centipede_controller（Direct；line 485）

### 生成引用
- `collapse_small`：shadowthrall_centipede_body, shadowthrall_centipede_head（Direct；line 55）

### 行为
- `brains/shadowthrall_centipede_brain.lua`：Wander（prefabs/shadowthrall_centipede.lua#shadowthrall_centipede_head）


## 函数

### CheckSpawnStatus  [430–443]
- 归属：shadowthrall_centipede_spawner

### ClearRecentlyCharged  [41–43]
- 归属：shadowthrall_centipede_body, shadowthrall_centipede_head

### DamageRedirectFn  [100–106]
- 归属：shadowthrall_centipede_body, shadowthrall_centipede_head

### DisplayNameFn  [88–90]
- 归属：shadowthrall_centipede_body, shadowthrall_centipede_head

### GetStatus  [168–172]
- 归属：shadowthrall_centipede_body, shadowthrall_centipede_head

### IsBackwardsLocomoting  [96–98]
- 归属：shadowthrall_centipede_body, shadowthrall_centipede_head

### IsFlipped  [131–133]
- 归属：shadowthrall_centipede_body, shadowthrall_centipede_head

### OnBlocked  [112–122]
- 归属：shadowthrall_centipede_body, shadowthrall_centipede_head

### OnBodyDeath  [358–360]
- 归属：shadowthrall_centipede_controller

### OnBrokeRockTree  [124–129]
- 归属：shadowthrall_centipede_body, shadowthrall_centipede_head

### OnCollide  [76–86]
- 归属：shadowthrall_centipede_body, shadowthrall_centipede_head

### OnDeath  [357–362]
- 归属：shadowthrall_centipede_controller

### OnLoad  [160–166]
- 归属：shadowthrall_centipede_body, shadowthrall_centipede_head

### OnOtherCollide  [47–72]
- 归属：shadowthrall_centipede_body, shadowthrall_centipede_head

### OnRift  [459–470]
- 归属：shadowthrall_centipede_spawner

### OnSave  [154–158]
- 归属：shadowthrall_centipede_body, shadowthrall_centipede_head

### OnSpawnLoad  [450–455]
- 归属：shadowthrall_centipede_spawner

### OnSpawnSave  [445–448]
- 归属：shadowthrall_centipede_spawner

### PlayIdleSound  [304–306]
- 归属：shadowthrall_centipede_head

### SetBackwardsLocomotion  [92–94]
- 归属：shadowthrall_centipede_body, shadowthrall_centipede_head

### SetSpikeVariation  [140–152]
- 归属：shadowthrall_centipede_body, shadowthrall_centipede_head

### SpawnPlanarEffectOn  [353–355]
- 归属：shadowthrall_centipede_controller

### SpawnSegments  [348–351]
- 归属：shadowthrall_centipede_controller

### TeleportOverrideFn  [108–110]
- 归属：shadowthrall_centipede_body, shadowthrall_centipede_head

### commonfn  [185–296]
- 归属：shadowthrall_centipede_body, shadowthrall_centipede_head

### controller_fn  [364–412]
- 归属：shadowthrall_centipede_controller

### headfn  [307–330]
- 归属：shadowthrall_centipede_head

### temp_spawnerfn  [418–483]
- 归属：shadowthrall_centipede_spawner

### torsofn  [337–342]
- 归属：shadowthrall_centipede_body

