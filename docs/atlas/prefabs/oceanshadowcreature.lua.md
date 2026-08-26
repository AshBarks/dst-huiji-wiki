# `prefabs/oceanshadowcreature.lua`

- 扫描角色：prefabs/oceanshadowcreature.lua
- 归属变体（3 个）：oceanhorror, oceanhorror_attachpivot, oceanhorror_ripples
## 关联

### 组件
- `components/combat.lua`：oceanhorror（Direct；line 413）
- `components/health.lua`：oceanhorror（Direct；line 407）
- `components/locomotor.lua`：oceanhorror（Direct；line 399）
- `components/lootdropper.lua`：oceanhorror（Direct；line 424）
- `components/sanityaura.lua`：oceanhorror（Direct；line 404）
- `components/shadowsubmissive.lua`：oceanhorror（Direct；line 422）
- `components/transparentonsanity.lua`：oceanhorror（Direct；line 369）

### 状态图
- `stategraphs/SGoceanshadowcreature.lua`：oceanhorror（Direct；line 439）

### 大脑
- `brains/oceanshadowcreaturebrain.lua`：oceanhorror（Direct；line 440）

### 预制体依赖
- `oceanhorror_attachpivot`：oceanhorror（Direct；line 511）
- `oceanhorror_ripples`：oceanhorror（Direct；line 511）
- `prefabs/nightmarefuel.lua`：oceanhorror（Direct；line 511）
- `shadow_teleport_in`：oceanhorror（Direct；line 511）
- `shadow_teleport_out`：oceanhorror（Direct；line 511）

### 行为
- `brains/oceanshadowcreaturebrain.lua`：StandAndAttack, StandStill, Wander（prefabs/oceanshadowcreature.lua#oceanhorror）


## 函数

### AttachToBoat  [80–119]
- 归属：oceanhorror

### CLIENT_ShadowSubmissive_HostileToPlayerTest  [320–333]
- 归属：oceanhorror

### CalcSanityAura  [230–235]
- 归属：oceanhorror

### DetachFromBoat  [121–141]
- 归属：oceanhorror

### EnableTeleportOnHit  [241–243]
- 归属：oceanhorror

### ExchangeWithTerrorBeak  [288–318]
- 归属：oceanhorror

### NotifyBrainOfTarget  [143–147]
- 归属：oceanhorror

### OnAlphaChanged  [271–275]
- 归属：oceanhorror, oceanhorror_ripples

### OnAttackOther  [277–279]
- 归属：oceanhorror

### OnAttacked  [245–254]
- 归属：oceanhorror

### OnDeath  [263–269]
- 归属：oceanhorror
- 掉落锚点：266:nightmarefuel

### OnNewCombatTarget  [256–261]
- 归属：oceanhorror

### OnRemove  [281–286]
- 归属：oceanhorror

### OnRipplesReplicated  [445–452]
- 归属：oceanhorror_ripples

### ShareTargetFn  [237–239]
- 归属：oceanhorror

### attachpivot_onsink  [482–486]
- 归属：oceanhorror_attachpivot

### attachpivotfn  [488–509]
- 归属：oceanhorror_attachpivot

### fn  [335–443]
- 归属：oceanhorror

### keeptargetfn  [181–222]
- 归属：oceanhorror

### onkilledbyother  [224–228]
- 归属：oceanhorror

### retargetfn  [149–178]
- 归属：oceanhorror

### ripplesfn  [454–480]
- 归属：oceanhorror_ripples

### update  [41–78]
- 归属：oceanhorror

