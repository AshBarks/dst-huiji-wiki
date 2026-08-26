# `prefabs/alterguardian_phase1.lua`

- 扫描角色：prefabs/alterguardian_phase1.lua
- 归属变体（3 个）：alterguardian_phase1, alterguardian_phase1_lunarrift, alterguardian_phase1_lunarrift_gestalt
## 关联

### 组件
- `components/colouradder.lua`：alterguardian_phase1_lunarrift（Direct；line 624）
- `components/colouraddersync.lua`：alterguardian_phase1_lunarrift（Direct；line 578）
- `components/combat.lua`：alterguardian_phase1, alterguardian_phase1_lunarrift（Direct；line 439）
- `components/drownable.lua`：alterguardian_phase1, alterguardian_phase1_lunarrift（Direct；line 476）
- `components/explosiveresist.lua`：alterguardian_phase1, alterguardian_phase1_lunarrift（Direct；line 449）
- `components/gestaltcapturable.lua`：alterguardian_phase1_lunarrift_gestalt（Direct；line 739）
- `components/hauntable.lua`：alterguardian_phase1, alterguardian_phase1_lunarrift（Direct；line 478）
- `components/health.lua`：alterguardian_phase1, alterguardian_phase1_lunarrift（Direct；line 435）
- `components/inspectable.lua`：alterguardian_phase1, alterguardian_phase1_lunarrift, alterguardian_phase1_lunarrift_gestalt（Direct；line 462,737）
- `components/knownlocations.lua`：alterguardian_phase1, alterguardian_phase1_lunarrift（Direct；line 465）
- `components/locomotor.lua`：alterguardian_phase1, alterguardian_phase1_lunarrift（Direct；line 429）
- `components/lootdropper.lua`：alterguardian_phase1, alterguardian_phase1_lunarrift（Direct；line 455）
- `components/planardamage.lua`：alterguardian_phase1_lunarrift（Direct；line 617）
- `components/planarentity.lua`：alterguardian_phase1_lunarrift（Direct；line 616）
- `components/sanityaura.lua`：alterguardian_phase1, alterguardian_phase1_lunarrift（Direct；line 451）
- `components/teleportedoverride.lua`：alterguardian_phase1, alterguardian_phase1_lunarrift（Direct；line 473）
- `components/timer.lua`：alterguardian_phase1, alterguardian_phase1_lunarrift（Direct；line 467）

### 状态图
- `stategraphs/SGalterguardian_phase1.lua`：alterguardian_phase1, alterguardian_phase1_lunarrift（Direct；line 432）

### 预制体依赖
- `alterguardian_phase1_lunarrift_gestalt`：alterguardian_phase1_lunarrift（Direct；line 753）
- `mining_moonglass_fx`：alterguardian_phase1, alterguardian_phase1_lunarrift（Direct；line 752,753）
- `prefabs/alterguardian_phase2.lua`：alterguardian_phase1（Direct；line 752）
- `prefabs/alterguardian_summon_fx.lua`：alterguardian_phase1（Direct；line 752）
- `prefabs/gestalt_alterguardian_projectile.lua`：alterguardian_phase1（Direct；line 752）
- `prefabs/moonrocknugget.lua`：alterguardian_phase1, alterguardian_phase1_lunarrift（Direct；line 752,753）
- `winter_ornament_boss_celestialrevenant`：alterguardian_phase1_lunarrift（Direct；line 753）


## 函数

### CalcSanityAura  [308–310]
- 归属：alterguardian_phase1, alterguardian_phase1_lunarrift

### CreatePlanarFx  [518–537]
- 归属：alterguardian_phase1_lunarrift

### DoGestaltSummon  [248–290]
- 归属：alterguardian_phase1, alterguardian_phase1_lunarrift

### EnableRollCollision  [211–219]
- 归属：alterguardian_phase1, alterguardian_phase1_lunarrift

### EnterShield  [292–300]
- 归属：alterguardian_phase1, alterguardian_phase1_lunarrift

### ExitShield  [302–306]
- 归属：alterguardian_phase1, alterguardian_phase1_lunarrift

### KeepTarget  [145–148]
- 归属：alterguardian_phase1, alterguardian_phase1_lunarrift

### OnAttacked  [158–162]
- 归属：alterguardian_phase1, alterguardian_phase1_lunarrift

### OnEntitySleep  [327–336]
- 归属：alterguardian_phase1, alterguardian_phase1_lunarrift

### OnEntityWake  [346–353]
- 归属：alterguardian_phase1, alterguardian_phase1_lunarrift

### OnLoad  [318–325]
- 归属：alterguardian_phase1, alterguardian_phase1_lunarrift

### OnMusicDirty  [87–95]
- 归属：alterguardian_phase1, alterguardian_phase1_lunarrift

### OnPhaseTransition  [164–177]
- 归属：alterguardian_phase1, alterguardian_phase1_lunarrift

### OnSave  [312–316]
- 归属：alterguardian_phase1, alterguardian_phase1_lunarrift

### PushMusic  [76–85]
- 归属：alterguardian_phase1, alterguardian_phase1_lunarrift

### Retarget  [120–142]
- 归属：alterguardian_phase1, alterguardian_phase1_lunarrift

### SetNoMusic  [97–101]
- 归属：alterguardian_phase1, alterguardian_phase1_lunarrift

### common_postinit_basic  [498–500]
- 归属：alterguardian_phase1

### common_postinit_rift  [562–594]
- 归属：alterguardian_phase1_lunarrift

### commonfn  [377–494]
- 归属：alterguardian_phase1, alterguardian_phase1_lunarrift

### find_gestalt_target  [221–243]
- 归属：alterguardian_phase1, alterguardian_phase1_lunarrift

### fn  [512–514]
- 归属：alterguardian_phase1

### gain_sleep_health  [339–344]
- 归属：alterguardian_phase1, alterguardian_phase1_lunarrift

### inspect_boss  [355–357]
- 归属：alterguardian_phase1, alterguardian_phase1_lunarrift

### on_timer_finished  [359–367]
- 归属：alterguardian_phase1, alterguardian_phase1_lunarrift

### oncollide  [202–209]
- 归属：alterguardian_phase1, alterguardian_phase1_lunarrift

### onothercollide  [179–199]
- 归属：alterguardian_phase1, alterguardian_phase1_lunarrift

### play_custom_hit  [104–114]
- 归属：alterguardian_phase1, alterguardian_phase1_lunarrift

### rift_EnableCameraFocus  [551–560]
- 归属：alterguardian_phase1_lunarrift

### rift_OnAddColourChanged  [539–541]
- 归属：alterguardian_phase1_lunarrift

### rift_OnCameraFocusDirty  [543–549]
- 归属：alterguardian_phase1_lunarrift

### rift_OnLoad  [600–607]
- 归属：alterguardian_phase1_lunarrift

### rift_OnRemoveEntity  [609–613]
- 归属：alterguardian_phase1_lunarrift

### rift_OnSave  [596–598]
- 归属：alterguardian_phase1_lunarrift

### riftfn  [641–643]
- 归属：alterguardian_phase1_lunarrift

### riftgestalt_AddFollowFx  [658–684]
- 归属：alterguardian_phase1_lunarrift_gestalt

### riftgestalt_OnCaptured  [647–652]
- 归属：alterguardian_phase1_lunarrift_gestalt

### riftgestalt_OnRemoveEntity  [654–656]
- 归属：alterguardian_phase1_lunarrift_gestalt

### riftgestaltfn  [692–748]
- 归属：alterguardian_phase1_lunarrift_gestalt

### server_postinit_basic  [502–510]
- 归属：alterguardian_phase1

### server_postinit_rift  [615–639]
- 归属：alterguardian_phase1_lunarrift

### teleport_override_fn  [150–156]
- 归属：alterguardian_phase1, alterguardian_phase1_lunarrift

