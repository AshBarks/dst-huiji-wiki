# `prefabs/daywalker2.lua`

- 扫描角色：prefabs/daywalker2.lua
- 归属变体（2 个）：daywalker2, daywalker2_buried_fx
## 关联

### 组件
- `components/bloomer.lua`：daywalker2（Direct；line 1199）
- `components/burnable.lua`：daywalker2（HelperExpanded；line 721）
- `components/colouradder.lua`：daywalker2（Direct；line 1198）
- `components/combat.lua`：daywalker2（Direct；line 1184）
- `components/despawnfader.lua`：daywalker2（Direct；line 1143）
- `components/drownable.lua`：daywalker2（Direct；line 1176）
- `components/entitytracker.lua`：daywalker2（Direct；line 1167）
- `components/epicscare.lua`：daywalker2（Direct；line 1214）
- `components/explosiveresist.lua`：daywalker2（Direct；line 1209）
- `components/freezable.lua`：daywalker2（HelperExpanded；line 722）
- `components/grouptargeter.lua`：daywalker2（Direct；line 1207）
- `components/health.lua`：daywalker2（Direct；line 1178）
- `components/healthtrigger.lua`：daywalker2（Direct；line 1201）
- `components/inspectable.lua`：daywalker2（Direct；line 1169）
- `components/knownlocations.lua`：daywalker2（Direct；line 1206）
- `components/locomotor.lua`：daywalker2（Direct；line 1172）
- `components/lootdropper.lua`：daywalker2（Direct；line 1217）
- `components/sanityaura.lua`：daywalker2（Direct；line 1211）
- `components/sleeper.lua`：daywalker2（Direct；line 726）
- `components/stuckdetection.lua`：daywalker2（Direct；line 1195）
- `components/talker.lua`：daywalker2（Direct；line 1130）
- `components/teleportedoverride.lua`：daywalker2（Direct；line 1226）
- `components/timer.lua`：daywalker2（Direct；line 1208）
- `components/updatelooper.lua`：daywalker2（Direct；line 165）

### 状态图
- `stategraphs/SGdaywalker2.lua`：daywalker2（Direct；line 851,1279）
- `stategraphs/SGdaywalker2_buried.lua`：daywalker2（Direct；line 821）

### 大脑
- `brains/daywalker2brain.lua`：daywalker2（Direct；line 854,1280）

### 预制体依赖
- `alterguardian_laserempty`：daywalker2（Direct；line 1341）
- `alterguardian_laserhit`：daywalker2（Direct；line 1341）
- `armorwagpunk_blueprint`：daywalker2（Direct；line 1341）
- `chesspiece_daywalker2_sketch`：daywalker2（Direct；line 1341）
- `chestupgrade_stacksize_blueprint`：daywalker2（Direct；line 1341）
- `daywalker2_armor1_break_fx`：daywalker2（Direct；line 1341）
- `daywalker2_armor2_break_fx`：daywalker2（Direct；line 1341）
- `daywalker2_buried_fx`：daywalker2（Direct；line 1341）
- `daywalker2_cannon_break_fx`：daywalker2（Direct；line 1341）
- `daywalker2_cloth_break_fx`：daywalker2（Direct；line 1341）
- `daywalker2_object_break_fx`：daywalker2（Direct；line 1341）
- `daywalker2_spike_break_fx`：daywalker2（Direct；line 1341）
- `daywalker2_spike_loot_fx`：daywalker2（Direct；line 1341）
- `daywalker2_swipe_fx`：daywalker2（Direct；line 1341）
- `junk_break_fx`：daywalker2（Direct；line 1341）
- `junkball_fx`：daywalker2（Direct；line 1341）
- `prefabs/alterguardian_laser.lua`：daywalker2（Direct；line 1341）
- `prefabs/gears.lua`：daywalker2（Direct；line 1341）
- `prefabs/wagpunk_bits.lua`：daywalker2（Direct；line 1341）
- `scrap_monoclehat`：daywalker2（Direct；line 1341）
- `wagpunkbits_kit`：daywalker2（Direct；line 1341）
- `wagpunkbits_kit_blueprint`：daywalker2（Direct；line 1341）
- `wagpunkhat_blueprint`：daywalker2（Direct；line 1341）
- `winter_ornament_boss_daywalker2`：daywalker2（Direct；line 1341）

### 行为
- `brains/daywalker2brain.lua`：ChaseAndAttackAndAvoid, FaceEntity, Leash, LeashAndAvoid, StandStill, Wander（prefabs/daywalker2.lua#daywalker2）


## 函数

### AddCombatStatusEffectComponents  [720–731]
- 归属：daywalker2

### CheckHealthPhase  [498–508]
- 归属：daywalker2

### CreateHead  [141–173]
- 归属：daywalker2

### DropItem  [365–392]
- 归属：daywalker2

### DropItemAsLoot  [394–396]
- 归属：daywalker2

### GetNextItem  [252–293]
- 归属：daywalker2

### GetStalking  [237–239]
- 归属：daywalker2

### GetStatus  [899–903]
- 归属：daywalker2

### IsStalking  [241–243]
- 归属：daywalker2

### KeepTargetFn  [569–578]
- 归属：daywalker2

### MakeBuried  [746–823]
- 归属：daywalker2

### MakeDefeated  [871–895]
- 归属：daywalker2

### MakeFreed  [825–869]
- 归属：daywalker2

### OnAttacked  [580–609]
- 归属：daywalker2

### OnDespawnTimer  [660–670]
- 归属：daywalker2

### OnEntitySleep  [954–962]
- 归属：daywalker2

### OnEntityWake  [964–969]
- 归属：daywalker2

### OnHeadTrackingDirty  [193–208]
- 归属：daywalker2

### OnItemUsed  [345–363]
- 归属：daywalker2

### OnJunkStolen  [685–710]
- 归属：daywalker2

### OnLoad  [922–952]
- 归属：daywalker2

### OnMinHealth  [654–658]
- 归属：daywalker2

### OnNewTarget  [611–623]
- 归属：daywalker2

### OnSave  [905–920]
- 归属：daywalker2

### OnStalkingDirty  [175–191]
- 归属：daywalker2

### OnTalk  [971–975]
- 归属：daywalker2

### OnTeleported  [1003–1016]
- 归属：daywalker2

### OnThiefDelayOver  [672–675]
- 归属：daywalker2

### OnThiefReset  [677–683]
- 归属：daywalker2

### PushMusic  [1062–1071]
- 归属：daywalker2

### RemoveCombatStatusEffectComponents  [733–742]
- 归属：daywalker2

### RetargetFn  [545–567]
- 归属：daywalker2

### SetEngaged  [625–647]
- 归属：daywalker2

### SetEquip  [295–343]
- 归属：daywalker2

### SetHeadTracking  [210–220]
- 归属：daywalker2

### SetStalking  [222–235]
- 归属：daywalker2

### ShouldSleep  [712–714]
- 归属：daywalker2

### ShouldWake  [716–718]
- 归属：daywalker2

### StartAttackCooldown  [649–652]
- 归属：daywalker2

### TestTackle  [400–461]
- 归属：daywalker2

### UpdateHead  [66–139]
- 归属：daywalker2

### UpdatePlayerTargets  [512–543]
- 归属：daywalker2

### buriedfx_OnEntityReplicated  [1295–1300]
- 归属：daywalker2_buried_fx

### buriedfx_OnRemoveEntity  [1288–1293]
- 归属：daywalker2_buried_fx

### buriedfx_fn  [1302–1337]
- 归属：daywalker2_buried_fx

### fn  [1088–1284]
- 归属：daywalker2

### lootsetfn  [1025–1058]
- 归属：daywalker2

### teleport_override_fn  [977–1001]
- 归属：daywalker2

