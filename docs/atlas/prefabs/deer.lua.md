# `prefabs/deer.lua`

- 扫描角色：prefabs/deer.lua
- 归属变体（5 个）：deer, deer_blue, deer_growantler_fx, deer_red, deer_unshackle_fx
## 关联

### 组件
- `components/burnable.lua`：deer, deer_blue, deer_red（HelperExpanded；line 548）
- `components/combat.lua`：deer, deer_blue, deer_red（Direct；line 511）
- `components/drownable.lua`：deer, deer_blue, deer_red（Direct；line 545）
- `components/entitytracker.lua`：deer, deer_blue, deer_red（Direct；line 565）
- `components/freezable.lua`：deer, deer_blue, deer_red（HelperExpanded；line 552）
- `components/hauntable.lua`：deer, deer_blue, deer_red（HelperExpanded；line 557）
- `components/health.lua`：deer, deer_blue, deer_red（Direct；line 505）
- `components/inspectable.lua`：deer, deer_blue, deer_red（Direct；line 538）
- `components/knownlocations.lua`：deer, deer_blue, deer_red（Direct；line 501）
- `components/locomotor.lua`：deer, deer_blue, deer_red（Direct；line 541）
- `components/lootdropper.lua`：deer, deer_blue, deer_red（Direct；line 535）
- `components/saltlicker.lua`：deer, deer_blue, deer_red（Direct；line 588）
- `components/sleeper.lua`：deer, deer_blue, deer_red（Direct；line 526）
- `components/spawnfader.lua`：deer, deer_blue, deer_red（Direct；line 462）
- `components/timer.lua`：deer, deer_blue, deer_red（Direct；line 500）

### 状态图
- `stategraphs/SGdeer.lua`：deer, deer_blue, deer_red（Direct；line 562）

### 大脑
- `brains/deerbrain.lua`：deer, deer_blue, deer_red（Direct；line 602）
- `brains/deergemmedbrain.lua`：deer, deer_blue, deer_red（Direct；line 586）

### 预制体依赖
- `bluegem`：deer_blue（Direct；line 702）
- `deer_fire_charge`：deer_red（Direct；line 701）
- `deer_fire_circle`：deer_red（Direct；line 701）
- `deer_growantler_fx`：deer（Direct；line 700）
- `deer_ice_charge`：deer_blue（Direct；line 702）
- `deer_ice_circle`：deer_blue（Direct；line 702）
- `deer_unshackle_fx`：deer_blue, deer_red（Direct；line 701,702）
- `deercorpse`：deer, deer_blue, deer_red（Direct；line 700,701,702）
- `meat`：deer, deer_blue, deer_red（Direct；line 700,701,702）
- `prefabs/boneshard.lua`：deer（Direct；line 700）
- `prefabs/deer_antler.lua`：deer（Direct；line 700）
- `redgem`：deer_red（Direct；line 701）

### 生成引用
- `collapse_small`：deer, deer_blue, deer_red（Direct；line 85）

### 行为
- `brains/deerbrain.lua`：AttackWall, Leash, Panic, RunAway, StandStill, Wander（prefabs/deer.lua#deer, prefabs/deer.lua#deer_blue, prefabs/deer.lua#deer_red）
- `brains/deergemmedbrain.lua`：AttackWall, ChaseAndAttack, FaceEntity, Leash, Panic, StandStill（prefabs/deer.lua#deer, prefabs/deer.lua#deer_blue, prefabs/deer.lua#deer_red）


## 函数

### DoBellIdleSound  [369–371]
- 归属：deer, deer_blue, deer_red, deer_unshackle_fx

### DoBellSound  [377–379]
- 归属：deer, deer_blue, deer_red, deer_unshackle_fx

### DoCast  [279–284]
- 归属：deer, deer_blue, deer_red

### DoChainIdleSound  [365–367]
- 归属：deer, deer_blue, deer_red

### DoChainSound  [373–375]
- 归属：deer, deer_blue, deer_red, deer_unshackle_fx

### DoNothing  [362–363]
- 归属：deer, deer_blue, deer_red

### FindCastTargets  [200–233]
- 归属：deer, deer_blue, deer_red

### GemmedOnAttacked  [307–313]
- 归属：deer, deer_blue, deer_red

### GemmedOnLoadPostPass  [349–354]
- 归属：deer, deer_blue, deer_red

### GemmedRetargetFn  [298–305]
- 归属：deer, deer_blue, deer_red

### GemmedShouldSleep  [181–183]
- 归属：deer, deer_blue, deer_red

### GemmedShouldWake  [185–187]
- 归属：deer, deer_blue, deer_red

### IsDeadKeeper  [292–296]
- 归属：deer, deer_blue, deer_red

### KeepTargetFn  [55–57]
- 归属：deer, deer_blue, deer_red

### NoSpellOverlap  [192–194]
- 归属：deer, deer_blue, deer_red

### OnAttacked  [63–66]
- 归属：deer, deer_blue, deer_red

### OnCollide  [90–96]
- 归属：deer, deer_blue, deer_red

### OnGotCommander  [329–338]
- 归属：deer, deer_blue, deer_red

### OnLostCommander  [340–347]
- 归属：deer, deer_blue, deer_red

### OnMigrate  [135–141]
- 归属：deer, deer_blue, deer_red

### OnNewTarget  [286–290]
- 归属：deer, deer_blue, deer_red

### OnShedAntler  [77–88]
- 归属：deer, deer_blue, deer_red
- 掉落锚点：79:

### OnUpdateOffset  [356–358]
- 归属：deer, deer_blue, deer_red

### SetEngaged  [315–327]
- 归属：deer, deer_blue, deer_red

### SetMigrating  [156–172]
- 归属：deer, deer_blue, deer_red

### SetupSounds  [381–398]
- 归属：deer, deer_blue, deer_red

### ShareTargetFn  [59–61]
- 归属：deer, deer_blue, deer_red

### ShowAntler  [98–105]
- 归属：deer, deer_blue, deer_red

### SpawnSpell  [235–240]
- 归属：deer, deer_blue, deer_red

### SpawnSpells  [242–277]
- 归属：deer, deer_blue, deer_red

### StartMigrationTask  [143–147]
- 归属：deer, deer_blue, deer_red

### StopMigrationTask  [149–154]
- 归属：deer, deer_blue, deer_red

### ValidShedAntlerTarget  [68–75]
- 归属：deer, deer_blue, deer_red

### bluefn  [616–618]
- 归属：deer_blue

### common_fn  [430–606]
- 归属：deer, deer_blue, deer_red

### fn  [608–610]
- 归属：deer

### getstatus  [416–418]
- 归属：deer, deer_blue, deer_red

### growantler_fn  [670–698]
- 归属：deer_growantler_fx

### ondeerherdmigration  [174–176]
- 归属：deer, deer_blue, deer_red

### onload  [407–414]
- 归属：deer, deer_blue, deer_red

### onqueuegrowantler  [127–131]
- 归属：deer, deer_blue, deer_red

### onsave  [402–405]
- 归属：deer, deer_blue, deer_red

### ontimerdone  [119–125]
- 归属：deer, deer_blue, deer_red

### redfn  [612–614]
- 归属：deer_red

### setantlered  [107–116]
- 归属：deer, deer_blue, deer_red

### unshackle_fn  [620–668]
- 归属：deer_unshackle_fx

