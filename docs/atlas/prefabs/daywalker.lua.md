# `prefabs/daywalker.lua`

- 扫描角色：prefabs/daywalker.lua
- 归属变体（1 个）：daywalker
## 关联

### 组件
- `components/combat.lua`：daywalker（Direct；line 1196）
- `components/despawnfader.lua`：daywalker（Direct；line 1150）
- `components/drownable.lua`：daywalker（Direct；line 1189）
- `components/entitytracker.lua`：daywalker（Direct；line 1180）
- `components/epicscare.lua`：daywalker（Direct；line 1220）
- `components/explosiveresist.lua`：daywalker（Direct；line 1215）
- `components/grouptargeter.lua`：daywalker（Direct；line 1213）
- `components/health.lua`：daywalker（Direct；line 1191）
- `components/healthtrigger.lua`：daywalker（Direct；line 1207）
- `components/inspectable.lua`：daywalker（Direct；line 1182）
- `components/knownlocations.lua`：daywalker（Direct；line 1212）
- `components/locomotor.lua`：daywalker（Direct；line 1185）
- `components/lootdropper.lua`：daywalker（Direct；line 1223）
- `components/sanityaura.lua`：daywalker（Direct；line 1217）
- `components/talker.lua`：daywalker（Direct；line 1135）
- `components/teleportedoverride.lua`：daywalker（Direct；line 1231）
- `components/timer.lua`：daywalker（Direct；line 1214）
- `components/updatelooper.lua`：daywalker（Direct；line 521）

### 状态图
- `stategraphs/SGdaywalker.lua`：daywalker（Direct；line 916,1294）
- `stategraphs/SGdaywalker_imprisoned.lua`：daywalker（Direct；line 889）

### 大脑
- `brains/daywalkerbrain.lua`：daywalker（Direct；line 952,1295）

### 预制体依赖
- `armordreadstone_blueprint`：daywalker（Direct；line 1301）
- `chesspiece_daywalker_sketch`：daywalker（Direct；line 1301）
- `daywalker_sinkhole`：daywalker（Direct；line 1301）
- `dreadstonehat_blueprint`：daywalker（Direct；line 1301）
- `prefabs/daywalker_pillar.lua`：daywalker（Direct；line 1301）
- `prefabs/horrorfuel.lua`：daywalker（Direct；line 1301）
- `prefabs/nightmarefuel.lua`：daywalker（Direct；line 1301）
- `prefabs/shadow_leech.lua`：daywalker（Direct；line 1301）
- `support_pillar_dreadstone_scaffold_blueprint`：daywalker（Direct；line 1301）
- `wall_dreadstone_item_blueprint`：daywalker（Direct；line 1301）
- `winter_ornament_boss_daywalker`：daywalker（Direct；line 1301）

### 生成引用
- `prefabs/shadow_leech.lua`：daywalker（Direct；line 380）

### 行为
- `brains/daywalkerbrain.lua`：ChaseAndAttack, FaceEntity, Leash, RunAway, Wander（prefabs/daywalker.lua#daywalker）


## 函数

### AttachLeech  [267–292]
- 归属：daywalker

### ClearTask  [294–296]
- 归属：daywalker

### CountPillars  [51–65]
- 归属：daywalker

### CreateChainBodyLink  [147–167]
- 归属：daywalker

### CreateEyeFlame  [397–419]
- 归属：daywalker

### CreateHead  [501–533]
- 归属：daywalker

### CreateShackleNeckBand  [126–145]
- 归属：daywalker

### DeltaFatigue  [831–839]
- 归属：daywalker

### DetachLeech  [298–356]
- 归属：daywalker

### EnableChains  [216–226]
- 归属：daywalker

### GetStalking  [619–621]
- 归属：daywalker

### GetStatus  [981–983]
- 归属：daywalker

### HasLeechAttached  [237–244]
- 归属：daywalker

### HasLeechTracked  [246–248]
- 归属：daywalker

### IsFatigued  [849–851]
- 归属：daywalker

### IsStalking  [623–625]
- 归属：daywalker

### KeepTargetFn  [749–753]
- 归属：daywalker

### LootSetupFn  [1096–1099]
- 归属：daywalker
- 掉落锚点：1097:

### MakeChained  [855–894]
- 归属：daywalker

### MakeDefeated  [957–977]
- 归属：daywalker

### MakeHarassed  [921–937]
- 归属：daywalker

### MakeHostile  [939–955]
- 归属：daywalker

### MakeUnchained  [896–919]
- 归属：daywalker

### OnAttachmentInterrupted  [358–366]
- 归属：daywalker

### OnAttacked  [755–764]
- 归属：daywalker

### OnChainSleepTask  [197–200]
- 归属：daywalker

### OnChainsDirty  [202–214]
- 归属：daywalker

### OnDespawnTimer  [808–818]
- 归属：daywalker

### OnEntitySleep  [1033–1044]
- 归属：daywalker

### OnEntityWake  [1046–1057]
- 归属：daywalker

### OnFacingModelDirty  [90–99]
- 归属：daywalker

### OnHeadTrackingDirty  [555–570]
- 归属：daywalker

### OnIncomingJump  [368–373]
- 归属：daywalker

### OnLoad  [990–1011]
- 归属：daywalker

### OnLoadPostPass  [1013–1031]
- 归属：daywalker

### OnMinHealth  [802–806]
- 归属：daywalker

### OnNewTarget  [766–773]
- 归属：daywalker

### OnPillarRemoved  [67–83]
- 归属：daywalker

### OnSave  [985–988]
- 归属：daywalker

### OnStalkingDirty  [535–553]
- 归属：daywalker

### OnStalkingNewState  [584–590]
- 归属：daywalker

### OnTalk  [1059–1063]
- 归属：daywalker

### PushMusic  [1083–1092]
- 归属：daywalker

### RegenFatigue  [822–829]
- 归属：daywalker

### RemoveChains  [184–195]
- 归属：daywalker

### ResetFatigue  [841–847]
- 归属：daywalker

### RetargetFn  [725–747]
- 归属：daywalker

### SetEngaged  [775–795]
- 归属：daywalker

### SetHeadTracking  [572–582]
- 归属：daywalker

### SetLeechAttached  [261–265]
- 归属：daywalker

### SetStalking  [592–617]
- 归属：daywalker

### SpawnChains  [169–182]
- 归属：daywalker

### SpawnLeeches  [375–393]
- 归属：daywalker

### StartAttackCooldown  [797–800]
- 归属：daywalker

### StartTrackingLeech  [250–259]
- 归属：daywalker

### SwitchToFacingModel  [101–122]
- 归属：daywalker

### UpdateHead  [425–499]
- 归属：daywalker

### UpdatePlayerTargets  [701–723]
- 归属：daywalker

### fn  [1103–1299]
- 归属：daywalker

### teleport_override_fn  [1065–1079]
- 归属：daywalker

