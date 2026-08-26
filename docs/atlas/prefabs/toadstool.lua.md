# `prefabs/toadstool.lua`

- 扫描角色：prefabs/toadstool.lua
- 归属变体（2 个）：toadstool, toadstool_dark
## 关联

### 组件
- `components/burnable.lua`：toadstool, toadstool_dark（HelperExpanded；line 937）
- `components/combat.lua`：toadstool, toadstool_dark（Direct；line 906）
- `components/drownable.lua`：toadstool, toadstool_dark（Direct；line 898）
- `components/epicscare.lua`：toadstool, toadstool_dark（Direct；line 919）
- `components/explosiveresist.lua`：toadstool, toadstool_dark（Direct；line 915）
- `components/freezable.lua`：toadstool, toadstool_dark（HelperExpanded；line 938）
- `components/groundpounder.lua`：toadstool, toadstool_dark（Direct；line 926）
- `components/grouptargeter.lua`：toadstool, toadstool_dark（Direct；line 924）
- `components/health.lua`：toadstool, toadstool_dark（Direct；line 900）
- `components/healthtrigger.lua`：toadstool, toadstool_dark（Direct；line 903）
- `components/inspectable.lua`：toadstool, toadstool_dark（Direct；line 882）
- `components/knownlocations.lua`：toadstool, toadstool_dark（Direct；line 935）
- `components/locomotor.lua`：toadstool, toadstool_dark（Direct；line 894）
- `components/lootdropper.lua`：toadstool, toadstool_dark（Direct；line 886）
- `components/sanityaura.lua`：toadstool, toadstool_dark（Direct；line 917）
- `components/sleeper.lua`：toadstool, toadstool_dark（Direct；line 888）
- `components/timer.lua`：toadstool, toadstool_dark（Direct；line 922）

### 状态图
- `stategraphs/SGtoadstool.lua`：toadstool, toadstool_dark（Direct；line 941）

### 大脑
- `brains/toadstoolbrain.lua`：toadstool, toadstool_dark（Direct；line 942）

### 预制体依赖
- `mushroom_light_blueprint`：toadstool（Direct；line 1046）
- `mushroombomb_dark_projectile`：toadstool_dark（Direct；line 1047）
- `mushroombomb_projectile`：toadstool（Direct；line 1046）
- `mushroomsprout_dark`：toadstool_dark（Direct；line 1047）
- `prefabs/mushroomsprout.lua`：toadstool（Direct；line 1046）
- `sleepbomb_blueprint`：toadstool_dark（Direct；line 1047）
- `toadstoolcorpse`：toadstool, toadstool_dark（Direct；line 1046,1047）

### 行为
- `brains/toadstoolbrain.lua`：ChaseAndAttack, Leash, Wander（prefabs/toadstool.lua#toadstool, prefabs/toadstool.lua#toadstool_dark）


## 函数

### AddSpecialLoot  [68–77]
- 归属：toadstool

### AddSpecialLootDark  [79–86]
- 归属：toadstool_dark

### AddSporeLoot  [60–66]
- 归属：toadstool, toadstool_dark

### AnnounceEscaped  [627–634]
- 归属：toadstool, toadstool_dark

### AnnounceWarning  [601–611]
- 归属：toadstool, toadstool_dark

### CalculateLevel  [428–433]
- 归属：toadstool, toadstool_dark

### CancelFade  [191–194]
- 归属：toadstool, toadstool_dark

### ClearRecentAttacker  [585–590]
- 归属：toadstool, toadstool_dark

### ClearRecentlyCharged  [775–777]
- 归属：toadstool, toadstool_dark

### DoMushroomBomb  [314–319]
- 归属：toadstool, toadstool_dark

### DoMushroomSprout  [361–424]
- 归属：toadstool, toadstool_dark

### DoSporeBomb  [239–243]
- 归属：toadstool, toadstool_dark

### DropShroomSkin  [697–700]
- 归属：toadstool, toadstool_dark

### EnterPhase2Trigger  [702–710]
- 归属：toadstool, toadstool_dark

### EnterPhase3Trigger  [712–720]
- 归属：toadstool

### EnterPhase3TriggerDark  [722–730]
- 归属：toadstool_dark

### EnterPhase4TriggerDark  [732–740]
- 归属：toadstool_dark

### FadeOut  [184–189]
- 归属：toadstool, toadstool_dark

### FindMushroomBombTargets  [253–289]
- 归属：toadstool, toadstool_dark

### FindMushroomSproutAngles  [323–333]
- 归属：toadstool, toadstool_dark

### FindSporeBombTargets  [200–237]
- 归属：toadstool, toadstool_dark

### KeepTargetFn  [559–562]
- 归属：toadstool, toadstool_dark

### NoHoles  [247–249]
- 归属：toadstool, toadstool_dark

### OnAttacked  [592–599]
- 归属：toadstool, toadstool_dark

### OnCollide  [797–806]
- 归属：toadstool, toadstool_dark

### OnDestroyOther  [779–795]
- 归属：toadstool, toadstool_dark
- 掉落锚点：787:

### OnEntitySleep  [662–667]
- 归属：toadstool, toadstool_dark

### OnEntityWake  [669–674]
- 归属：toadstool, toadstool_dark

### OnEscaped  [636–648]
- 归属：toadstool, toadstool_dark

### OnFadeDirty  [177–182]
- 归属：toadstool, toadstool_dark

### OnFleeWarning  [613–625]
- 归属：toadstool, toadstool_dark

### OnLinkMushroomSprout  [473–480]
- 归属：toadstool, toadstool_dark

### OnLoad  [748–771]
- 归属：toadstool, toadstool_dark

### OnNewState  [577–583]
- 归属：toadstool, toadstool_dark

### OnNewTarget  [564–575]
- 归属：toadstool, toadstool_dark

### OnSave  [742–746]
- 归属：toadstool, toadstool_dark

### OnUnlinkMushroomSprout  [464–471]
- 归属：toadstool, toadstool_dark

### OnUpdateFade  [153–175]
- 归属：toadstool, toadstool_dark

### PushMusic  [814–823]
- 归属：toadstool, toadstool_dark

### RetargetFn  [510–557]
- 归属：toadstool, toadstool_dark

### SetPhaseLevel  [682–695]
- 归属：toadstool, toadstool_dark

### ShouldSleep  [652–654]
- 归属：toadstool, toadstool_dark

### ShouldWake  [656–658]
- 归属：toadstool, toadstool_dark

### SpawnMushroomBombProjectile  [291–312]
- 归属：toadstool, toadstool_dark

### SproutLaunch  [335–350]
- 归属：toadstool, toadstool_dark

### UpdateLevel  [435–462]
- 归属：toadstool, toadstool_dark

### UpdatePlayerTargets  [484–506]
- 归属：toadstool, toadstool_dark

### common_fn  [827–994]
- 归属：toadstool, toadstool_dark

### dark_fn  [1019–1044]
- 归属：toadstool_dark

### getstatus  [810–812]
- 归属：toadstool, toadstool_dark

### normal_fn  [996–1017]
- 归属：toadstool

