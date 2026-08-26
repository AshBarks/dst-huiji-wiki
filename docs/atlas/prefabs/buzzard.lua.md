# `prefabs/buzzard.lua`

- 扫描角色：prefabs/buzzard.lua
- 归属变体（2 个）：buzzard, mutatedbuzzard_gestalt
## 关联

### 组件
- `components/burnable.lua`：buzzard, mutatedbuzzard_gestalt（HelperExpanded；line 143,475）
- `components/combat.lua`：buzzard, mutatedbuzzard_gestalt（Direct；line 107,449）
- `components/drownable.lua`：buzzard, mutatedbuzzard_gestalt（Direct；line 136,487）
- `components/eater.lua`：buzzard, mutatedbuzzard_gestalt（Direct；line 116,459）
- `components/freezable.lua`：buzzard, mutatedbuzzard_gestalt（HelperExpanded；line 144,476）
- `components/hauntable.lua`：buzzard, mutatedbuzzard_gestalt（Direct/HelperExpanded；line 155,516）
- `components/health.lua`：buzzard, mutatedbuzzard_gestalt（Direct；line 102,446）
- `components/inspectable.lua`：buzzard, mutatedbuzzard_gestalt（Direct；line 131,465）
- `components/knownlocations.lua`：buzzard, mutatedbuzzard_gestalt（Direct；line 135,468）
- `components/locomotor.lua`：buzzard, mutatedbuzzard_gestalt（Direct；line 148,478）
- `components/lootdropper.lua`：buzzard, mutatedbuzzard_gestalt（Direct；line 126,462）
- `components/planardamage.lua`：mutatedbuzzard_gestalt（Direct；line 472）
- `components/planarentity.lua`：mutatedbuzzard_gestalt（Direct；line 470）
- `components/sleeper.lua`：buzzard（Direct；line 121）
- `components/timer.lua`：mutatedbuzzard_gestalt（Direct；line 482）

### 状态图
- `stategraphs/SGbuzzard.lua`：buzzard, mutatedbuzzard_gestalt（Direct；line 152,508）

### 大脑
- `brains/buzzardbrain.lua`：buzzard, mutatedbuzzard_gestalt（Direct；line 153,509）

### 预制体依赖
- `-- Not loaded for normal, spawner handles it.
    "circlingbuzzard_lunar"`：mutatedbuzzard_gestalt（Direct；line 528）
- `--"feather_crow", --Unique feather?
    "warg_mutated_breath_fx"`：mutatedbuzzard_gestalt（Direct；line 528）
- `buzzardcorpse`：buzzard（Direct；line 527）
- `drumstick`：buzzard（Direct；line 527）
- `feather_crow`：buzzard（Direct；line 527）
- `smallmeat`：buzzard, mutatedbuzzard_gestalt（Direct；line 527,528）
- `spoiled_food`：mutatedbuzzard_gestalt（Direct；line 528）
- `warg_mutated_ember_fx`：mutatedbuzzard_gestalt（Direct；line 528）

### 行为
- `brains/buzzardbrain.lua`：StandAndAttack, Wander（prefabs/buzzard.lua#buzzard, prefabs/buzzard.lua#mutatedbuzzard_gestalt）


## 函数

### ClearMigrationTask  [318–323]
- 归属：mutatedbuzzard_gestalt

### KeepTargetFn  [26–29]
- 归属：buzzard

### LoseCorpseOwnership  [293–297]
- 归属：mutatedbuzzard_gestalt

### Mutated_AddSharedTargetRef  [272–275]
- 归属：mutatedbuzzard_gestalt

### Mutated_CanSuggestTargetFn  [365–367]
- 归属：mutatedbuzzard_gestalt

### Mutated_EnterMigration  [311–316]
- 归属：mutatedbuzzard_gestalt

### Mutated_GetStatus  [389–393]
- 归属：mutatedbuzzard_gestalt

### Mutated_IsValidAlly  [340–342]
- 归属：mutatedbuzzard_gestalt

### Mutated_KeepTargetFn  [265–268]
- 归属：mutatedbuzzard_gestalt

### Mutated_OnAttacked  [344–346]
- 归属：mutatedbuzzard_gestalt

### Mutated_OnDeath  [299–309]
- 归属：mutatedbuzzard_gestalt

### Mutated_OnDroppedTarget  [361–363]
- 归属：mutatedbuzzard_gestalt

### Mutated_OnEntitySleep  [325–331]
- 归属：mutatedbuzzard_gestalt

### Mutated_OnEntityWake  [333–335]
- 归属：mutatedbuzzard_gestalt

### Mutated_OnNewCombatTarget  [348–359]
- 归属：mutatedbuzzard_gestalt

### Mutated_OnRemoveEntity  [369–387]
- 归属：mutatedbuzzard_gestalt

### Mutated_RemoveSharedTargetRef  [277–285]
- 归属：mutatedbuzzard_gestalt

### Mutated_RetargetFn  [261–263]
- 归属：（未归属）

### Mutated_SetFlameThrowerOnCd  [253–256]
- 归属：mutatedbuzzard_gestalt

### Mutated_SwitchToEightFaced  [227–238]
- 归属：mutatedbuzzard_gestalt

### Mutated_SwitchToFourFaced  [240–251]
- 归属：mutatedbuzzard_gestalt

### OnAttacked  [31–33]
- 归属：buzzard

### OnEntitySleep  [42–44]
- 归属：buzzard, mutatedbuzzard_gestalt

### OnHaunt  [46–51]
- 归属：buzzard

### OnPreLoad  [35–40]
- 归属：buzzard, mutatedbuzzard_gestalt

### SetOwnCorpse  [287–291]
- 归属：mutatedbuzzard_gestalt

### fn  [68–164]
- 归属：buzzard

### mutated_fn  [396–525]
- 归属：mutatedbuzzard_gestalt

