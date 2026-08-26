# `prefabs/vault_pillar_guard.lua`

- 扫描角色：prefabs/vault_pillar_guard.lua
- 归属变体（2 个）：vault_pillar_guard, vault_pillar_guard_dormant
## 关联

### 组件
- `components/combat.lua`：vault_pillar_guard（Direct；line 400）
- `components/damagetypebonus.lua`：vault_pillar_guard（Direct；line 397）
- `components/damagetyperesist.lua`：vault_pillar_guard（Direct；line 398）
- `components/explosiveresist.lua`：vault_pillar_guard（Direct；line 430）
- `components/freezable.lua`：vault_pillar_guard（HelperExpanded；line 434）
- `components/hauntable.lua`：vault_pillar_guard（HelperExpanded；line 435）
- `components/health.lua`：vault_pillar_guard（Direct；line 393）
- `components/healthtrigger.lua`：vault_pillar_guard（Direct；line 410）
- `components/inspectable.lua`：vault_pillar_guard, vault_pillar_guard_dormant（Direct；line 387,495）
- `components/knownlocations.lua`：vault_pillar_guard（Direct；line 432）
- `components/locomotor.lua`：vault_pillar_guard（Direct；line 389）
- `components/lootdropper.lua`：vault_pillar_guard（Direct；line 418）
- `components/teleportedoverride.lua`：vault_pillar_guard（Direct；line 427）
- `components/timer.lua`：vault_pillar_guard（Direct；line 416）

### 状态图
- `stategraphs/SGvault_pillar_guard.lua`：vault_pillar_guard（Direct；line 441）

### 大脑
- `brains/vault_pillar_guardbrain.lua`：vault_pillar_guard（Direct；line 442）

### 预制体依赖
- `--loot
	"thulecite"`：vault_pillar_guard（Direct；line 502）
- `prefabs/moonrocknugget.lua`：vault_pillar_guard（Direct；line 502）
- `prefabs/rocks.lua`：vault_pillar_guard（Direct；line 502）
- `prefabs/temp_beta_msg.lua`：vault_pillar_guard（Direct；line 502）
- `prefabs/thulecite_pieces.lua`：vault_pillar_guard（Direct；line 502）
- `prefabs/vault_orb_fragment.lua`：vault_pillar_guard（Direct；line 502）
- `prefabs/vault_pillar_guard.lua`：vault_pillar_guard_dormant（Direct；line 503）
- `vault_orb_refined_blueprint`：vault_pillar_guard（Direct；line 502）
- `vault_pillar_guard_swipe_fx`：vault_pillar_guard（Direct；line 502）

### 行为
- `brains/vault_pillar_guardbrain.lua`：ChaseAndAttack, FaceEntity, Wander（prefabs/vault_pillar_guard.lua#vault_pillar_guard）


## 函数

### ActivatePillarGuard  [449–465]
- 归属：vault_pillar_guard_dormant

### CreateDebris  [83–102]
- 归属：vault_pillar_guard

### DetachDebris  [104–132]
- 归属：vault_pillar_guard

### IsClosestToTarget  [182–193]
- 归属：vault_pillar_guard

### KeepTargetFn  [238–243]
- 归属：vault_pillar_guard

### LootSetupFn  [273–278]
- 归属：vault_pillar_guard
- 掉落锚点：275:

### OnAttacked  [245–271]
- 归属：vault_pillar_guard

### OnDebrisDirty  [134–155]
- 归属：vault_pillar_guard

### OnLoad  [331–340]
- 归属：vault_pillar_guard

### RecycleDebris  [66–81]
- 归属：vault_pillar_guard

### RetargetFn  [195–236]
- 归属：vault_pillar_guard

### TriggerDebris  [157–169]
- 归属：vault_pillar_guard

### dormantfn  [467–500]
- 归属：vault_pillar_guard_dormant

### fn  [342–447]
- 归属：vault_pillar_guard

### teleport_override_fn  [173–178]
- 归属：vault_pillar_guard

