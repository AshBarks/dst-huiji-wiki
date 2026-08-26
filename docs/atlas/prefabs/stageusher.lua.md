# `prefabs/stageusher.lua`

- 扫描角色：prefabs/stageusher.lua
- 归属变体（3 个）：stageusher, stageusher_attackarm, stageusher_attackhand
## 关联

### 组件
- `components/burnable.lua`：stageusher（Direct；line 220）
- `components/combat.lua`：stageusher（Direct；line 251）
- `components/health.lua`：stageusher（Direct；line 246）
- `components/inspectable.lua`：stageusher（Direct；line 232）
- `components/knownlocations.lua`：stageusher（Direct；line 236）
- `components/locomotor.lua`：stageusher（Direct；line 226）
- `components/sanityaura.lua`：stageusher_attackhand（Direct；line 492）
- `components/stretcher.lua`：stageusher_attackarm（Direct；line 540）
- `components/updatelooper.lua`：stageusher_attackhand（Direct；line 441）
- `components/workable.lua`：stageusher（Direct；line 239）

### 状态图
- `stategraphs/SGstageusher.lua`：stageusher（Direct；line 275）

### 大脑
- `brains/stageusherbrain.lua`：stageusher（Direct；line 276）

### 预制体依赖
- `stageusher_attackarm`：stageusher（Direct；line 550）
- `stageusher_attackhand`：stageusher（Direct；line 550）

### 行为
- `brains/stageusherbrain.lua`：ChaseAndAttack, DoAction, StandStill, Wander（prefabs/stageusher.lua#stageusher）


## 函数

### ChangeStanding  [99–114]
- 归属：stageusher

### GetStatus  [117–119]
- 归属：stageusher

### IsStanding  [95–97]
- 归属：stageusher

### OnLoad  [159–167]
- 归属：stageusher

### OnSave  [155–157]
- 归属：stageusher

### SetCreepTarget  [436–458]
- 归属：stageusher_attackhand

### SetOwner  [387–390]
- 归属：stageusher_attackhand

### SetPhysicsState  [46–69]
- 归属：stageusher

### StartAttackingTarget  [72–92]
- 归属：stageusher

### armfn  [518–548]
- 归属：stageusher_attackarm

### create_shadow_arm  [287–307]
- 归属：stageusher_attackhand

### fn  [172–283]
- 归属：stageusher

### hand_dissipate  [356–372]
- 归属：（未归属）

### handfn  [462–515]
- 归属：stageusher_attackhand

### new_creep  [313–316]
- 归属：stageusher_attackhand

### on_attacked  [148–152]
- 归属：stageusher

### on_dropped_target  [141–146]
- 归属：stageusher

### on_giveup_timer_done  [122–127]
- 归属：stageusher

### on_grab_anim_over  [339–354]
- 归属：（未归属）

### on_new_combat_target  [136–139]
- 归属：stageusher

### restart_giveup_timer  [129–134]
- 归属：stageusher

### start_new_creep  [318–336]
- 归属：（未归属）

### test_for_damage_targets  [399–434]
- 归属：stageusher_attackhand

### usher_keep_target  [33–36]
- 归属：stageusher

### usher_onfinishedworking  [28–30]
- 归属：stageusher

### usher_onworked  [24–26]
- 归属：stageusher

### usher_should_aggro  [38–43]
- 归属：stageusher

