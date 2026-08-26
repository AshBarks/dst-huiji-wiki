# `prefabs/bearger.lua`

- 扫描角色：prefabs/bearger.lua
- 归属变体（2 个）：bearger, mutatedbearger
## 关联

### 组件
- `components/burnable.lua`：bearger, mutatedbearger（HelperExpanded；line 527）
- `components/combat.lua`：bearger, mutatedbearger（Direct；line 476）
- `components/drownable.lua`：bearger, mutatedbearger（Direct；line 518）
- `components/eater.lua`：bearger（Direct；line 569）
- `components/explosiveresist.lua`：bearger, mutatedbearger（Direct；line 483）
- `components/freezable.lua`：bearger, mutatedbearger（HelperExpanded；line 528）
- `components/groundpounder.lua`：bearger, mutatedbearger（Direct；line 504）
- `components/health.lua`：bearger, mutatedbearger（Direct；line 472）
- `components/inspectable.lua`：bearger, mutatedbearger（Direct；line 498）
- `components/inventory.lua`：bearger（Direct；line 568）
- `components/knownlocations.lua`：bearger, mutatedbearger（Direct；line 503）
- `components/locomotor.lua`：bearger, mutatedbearger（Direct；line 544）
- `components/lootdropper.lua`：bearger, mutatedbearger（Direct；line 494）
- `components/planardamage.lua`：mutatedbearger（Direct；line 825）
- `components/planarentity.lua`：mutatedbearger（Direct；line 824）
- `components/sanityaura.lua`：bearger, mutatedbearger（Direct；line 467）
- `components/shedder.lua`：bearger, mutatedbearger（Direct；line 487）
- `components/sleeper.lua`：bearger（Direct；line 583）
- `components/thief.lua`：bearger（Direct；line 567）
- `components/timer.lua`：bearger, mutatedbearger（Direct；line 514）

### 状态图
- `stategraphs/SGbearger.lua`：bearger, mutatedbearger（Direct；line 549）

### 大脑
- `brains/beargerbrain.lua`：bearger, mutatedbearger（Direct；line 550）

### 预制体依赖
- `bearger_sinkhole`：mutatedbearger（Direct；line 846）
- `bearger_swipe_fx`：bearger（Direct；line 845）
- `beargercorpse`：bearger（Direct；line 845）
- `chesspiece_bearger_mutated_sketch`：mutatedbearger（Direct；line 846）
- `chesspiece_bearger_sketch`：bearger（Direct；line 845）
- `collapse_small`：bearger, mutatedbearger（Direct；line 845,846）
- `groundpound_fx`：bearger, mutatedbearger（Direct；line 845,846）
- `groundpoundring_fx`：bearger, mutatedbearger（Direct；line 845,846）
- `meat`：bearger（Direct；line 845）
- `mutatedbearger_swipe_fx`：mutatedbearger（Direct；line 846）
- `prefabs/bearger.lua`：mutatedbearger（Direct；line 846）
- `prefabs/bearger_fur.lua`：bearger（Direct；line 845）
- `prefabs/coolant.lua`：mutatedbearger（Direct；line 846）
- `prefabs/furtuft.lua`：bearger, mutatedbearger（Direct；line 845,846）
- `prefabs/purebrilliance.lua`：mutatedbearger（Direct；line 846）
- `spoiled_food`：mutatedbearger（Direct；line 846）
- `winter_ornament_boss_mutatedbearger`：mutatedbearger（Direct；line 846）

### 行为
- `brains/beargerbrain.lua`：ChaseAndAttack, ChaseAndRam, DoAction, Wander（prefabs/bearger.lua#bearger, prefabs/bearger.lua#mutatedbearger）


## 函数

### CalcSanityAura  [103–105]
- 归属：bearger, mutatedbearger

### ClearRecentlyCharged  [197–199]
- 归属：bearger, mutatedbearger

### HoneyedItem  [107–109]
- 归属：bearger

### IsHibernationSeason  [181–183]
- 归属：bearger

### IsStandState  [331–333]
- 归属：bearger, mutatedbearger

### KeepTargetFn  [156–158]
- 归属：bearger, mutatedbearger

### LaunchItem  [248–259]
- 归属：bearger, mutatedbearger

### LootSetupFn_mutated  [754–757]
- 归属：mutatedbearger
- 掉落锚点：755:

### Mutated_CreateEyeFlame  [683–705]
- 归属：mutatedbearger

### Mutated_CreateGestaltFlame  [658–681]
- 归属：mutatedbearger

### Mutated_IsButtRecovering  [726–728]
- 归属：mutatedbearger

### Mutated_OnDead  [402–407]
- 归属：mutatedbearger

### Mutated_OnRecoveryHealthDelta  [707–715]
- 归属：mutatedbearger

### Mutated_OnTemp8Faced  [626–636]
- 归属：mutatedbearger

### Mutated_PushMusic  [730–741]
- 归属：mutatedbearger

### Mutated_StartButtRecovery  [717–724]
- 归属：mutatedbearger

### Mutated_SwitchToEightFaced  [638–646]
- 归属：mutatedbearger

### Mutated_SwitchToFourFaced  [648–656]
- 归属：mutatedbearger

### OnAttacked  [193–195]
- 归属：bearger, mutatedbearger

### OnCollide  [219–229]
- 归属：bearger, mutatedbearger

### OnCombatTarget  [315–324]
- 归属：bearger

### OnDead  [335–339]
- 归属：bearger

### OnDestroyOther  [201–217]
- 归属：bearger, mutatedbearger
- 掉落锚点：209:

### OnDroppedTarget  [309–313]
- 归属：bearger

### OnGroundPound  [261–265]
- 归属：bearger, mutatedbearger

### OnHitOther  [267–280]
- 归属：bearger, mutatedbearger

### OnKilledOther  [395–400]
- 归属：bearger

### OnLoad  [165–179]
- 归属：bearger, mutatedbearger

### OnPlayerAction  [345–364]
- 归属：bearger

### OnPlayerJoined  [368–377]
- 归属：bearger

### OnPlayerLeft  [379–387]
- 归属：bearger

### OnRemove  [341–343]
- 归属：bearger

### OnSave  [160–163]
- 归属：bearger, mutatedbearger

### OnSeasonChange  [185–191]
- 归属：bearger

### OnWakeUp  [391–393]
- 归属：bearger

### RetargetFn_Mutated  [144–154]
- 归属：mutatedbearger

### RetargetFn_Normal  [115–142]
- 归属：bearger

### SetStandState  [326–329]
- 归属：bearger, mutatedbearger

### ShouldSleep  [282–296]
- 归属：bearger

### ShouldWake  [298–307]
- 归属：bearger

### SwitchToEightFaced  [409–414]
- 归属：bearger, mutatedbearger

### SwitchToFourFaced  [416–421]
- 归属：bearger, mutatedbearger

### WorkEntities  [233–246]
- 归属：bearger, mutatedbearger

### commonfn  [423–553]
- 归属：bearger, mutatedbearger

### mutatedcommonfn  [759–794]
- 归属：mutatedbearger

### mutatedfn  [796–842]
- 归属：mutatedbearger

### normalfn  [555–622]
- 归属：bearger

