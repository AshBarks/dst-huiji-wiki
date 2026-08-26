# `prefabs/winona_catapult.lua`

- 扫描角色：prefabs/winona_catapult.lua
- 归属变体（2 个）：winona_catapult, winona_catapult_item
## 关联

### 组件
- `components/activatable.lua`：winona_catapult（Direct；line 864）
- `components/burnable.lua`：winona_catapult, winona_catapult_item（HelperExpanded；line 1143,1277）
- `components/circuitnode.lua`：winona_catapult（Direct；line 1114）
- `components/colouradder.lua`：winona_catapult（Direct；line 1090）
- `components/combat.lua`：winona_catapult（Direct；line 1096）
- `components/damagetypebonus.lua`：winona_catapult（Direct；line 1104）
- `components/deployable.lua`：winona_catapult_item（Direct；line 1269）
- `components/deployhelper.lua`：winona_catapult（Direct；line 1056）
- `components/floater.lua`：winona_catapult_item（HelperExpanded；line 1256）
- `components/hauntable.lua`：winona_catapult, winona_catapult_item（Direct/HelperExpanded；line 1142,1274）
- `components/health.lua`：winona_catapult（Direct；line 1092）
- `components/inspectable.lua`：winona_catapult, winona_catapult_item（Direct；line 1086,1266）
- `components/inventoryitem.lua`：winona_catapult_item（Direct；line 1267）
- `components/lootdropper.lua`：winona_catapult（Direct；line 1106）
- `components/placer.lua`：winona_catapult, winona_catapult_item（HelperExpanded；line 1286）
- `components/planardamage.lua`：winona_catapult（Direct；line 1103）
- `components/portablestructure.lua`：winona_catapult（Direct；line 1083）
- `components/powerload.lua`：winona_catapult（Direct；line 1122）
- `components/propagator.lua`：winona_catapult, winona_catapult_item（HelperExpanded；line 1144,1278）
- `components/savedrotation.lua`：winona_catapult（Direct；line 1112）
- `components/timer.lua`：winona_catapult（Direct；line 1125）
- `components/updatelooper.lua`：winona_catapult（Direct；line 1089）
- `components/workable.lua`：winona_catapult（Direct；line 1107）

### 状态图
- `stategraphs/SGwinona_catapult.lua`：winona_catapult（Direct；line 1160）

### 大脑
- `brains/winonacatapultbrain.lua`：winona_catapult（Direct；line 789）

### 预制体依赖
- `collapse_small`：winona_catapult（Direct；line 1285）
- `prefabs/winona_battery_sparks.lua`：winona_catapult（Direct；line 1285）
- `prefabs/winona_catapult.lua`：winona_catapult_item（Direct；line 1287）
- `prefabs/winona_catapult_projectile.lua`：winona_catapult（Direct；line 1285）
- `winona_catapult_item`：winona_catapult（Direct；line 1285）

### 行为
- `brains/winonacatapultbrain.lua`：StandAndAttack（prefabs/winona_catapult.lua#winona_catapult）


## 函数

### AddBatteryPower  [855–873]
- 归属：winona_catapult

### ApplySkillBonuses  [49–52]
- 归属：winona_catapult, winona_catapult_item

### CalcAoeRadiusMult  [33–35]
- 归属：winona_catapult, winona_catapult_item

### CalcSleepModeDelay  [37–39]
- 归属：winona_catapult

### CancelLedBlink  [609–619]
- 归属：winona_catapult

### CancelLedRapidBlink  [621–627]
- 归属：winona_catapult

### ChangeToItem  [199–209]
- 归属：winona_catapult

### ConfigureSkillTreeUpgrades  [54–76]
- 归属：winona_catapult, winona_catapult_item

### CreateElementalRock  [952–972]
- 归属：winona_catapult

### CreatePlacerBatteryRing  [465–489]
- 归属：winona_catapult

### CreatePlacerCatapult  [1184–1206]
- 归属：（未归属）

### CreatePlacerRing  [491–518]
- 归属：winona_catapult

### DoBuiltOrDeployed  [282–293]
- 归属：winona_catapult, winona_catapult_item

### DoLedRapidBlinkOn  [638–646]
- 归属：winona_catapult

### DoWireSparks  [906–916]
- 归属：winona_catapult

### ForceDropTarget  [166–170]
- 归属：（未归属）

### GetStatus  [577–582]
- 归属：winona_catapult

### HasPowerAlignment  [723–730]
- 归属：winona_catapult

### IsActiveMode  [883–885]
- 归属：winona_catapult

### IsPowered  [875–877]
- 归属：winona_catapult

### IsTargetTooClose  [84–86]
- 归属：winona_catapult

### IsTargetTooFar  [80–82]
- 归属：winona_catapult

### IsTargetTooFarOrTooClose  [88–100]
- 归属：winona_catapult

### OnActivate  [831–846]
- 归属：winona_catapult

### OnActiveWakeup  [887–892]
- 归属：winona_catapult

### OnAllowReactivate  [825–829]
- 归属：winona_catapult

### OnAttacked  [181–197]
- 归属：winona_catapult

### OnAutoActiveTaskEnded  [799–801]
- 归属：winona_catapult

### OnBuilt  [295–297]
- 归属：winona_catapult

### OnBurnt  [255–280]
- 归属：winona_catapult

### OnCatapultSpeedBoost  [709–718]
- 归属：winona_catapult

### OnCircuitChanged  [732–750]
- 归属：winona_catapult

### OnConnectCircuit  [918–931]
- 归属：winona_catapult

### OnDeath  [231–253]
- 归属：winona_catapult

### OnDeploy  [1227–1237]
- 归属：winona_catapult_item

### OnDisconnectCircuit  [933–950]
- 归属：winona_catapult

### OnDismantle  [299–304]
- 归属：winona_catapult

### OnDroppedTarget  [177–179]
- 归属：winona_catapult

### OnElementDirty  [974–994]
- 归属：winona_catapult

### OnEnableHelper  [520–567]
- 归属：winona_catapult

### OnEntitySleep  [667–674]
- 归属：winona_catapult

### OnEntityWake  [676–681]
- 归属：winona_catapult

### OnHealthDelta  [308–314]
- 归属：winona_catapult

### OnInit  [370–373]
- 归属：winona_catapult

### OnLedBlink  [661–665]
- 归属：winona_catapult

### OnLedRapidBlink  [629–636]
- 归属：winona_catapult

### OnLoad  [329–361]
- 归属：winona_catapult

### OnLoadPostPass  [363–368]
- 归属：winona_catapult

### OnNewCombatTarget  [172–175]
- 归属：winona_catapult

### OnReadyForConnection  [803–823]
- 归属：winona_catapult

### OnSave  [316–327]
- 归属：winona_catapult

### OnStartAttack  [996–1008]
- 归属：winona_catapult

### OnStartHelper  [569–573]
- 归属：winona_catapult

### OnTimerDone  [699–707]
- 归属：winona_catapult

### OnUpdateElementalVolleyHelper  [414–446]
- 归属：winona_catapult

### OnUpdatePlacerHelper  [379–394]
- 归属：winona_catapult

### OnUpdateSparks  [894–904]
- 归属：winona_catapult

### OnUpdateVolleyHelper  [396–412]
- 归属：winona_catapult

### OnUpdateWakeUpHelper  [448–463]
- 归属：winona_catapult

### OnWorked  [211–214]
- 归属：winona_catapult

### OnWorkedBurnt  [216–229]
- 归属：winona_catapult

### OverrideActivateVerb  [879–881]
- 归属：winona_catapult

### PowerOff  [848–853]
- 归属：winona_catapult

### RefreshAttackPeriod  [41–47]
- 归属：winona_catapult, winona_catapult_item

### RetargetFn  [106–147]
- 归属：winona_catapult

### SetActiveMode  [752–797]
- 归属：winona_catapult

### SetLedEnabled  [587–607]
- 归属：winona_catapult

### SetLedStatusBlink  [683–697]
- 归属：winona_catapult

### SetLedStatusOff  [655–659]
- 归属：winona_catapult

### SetLedStatusOn  [648–653]
- 归属：winona_catapult

### ShareTargetFn  [155–157]
- 归属：winona_catapult

### ShouldAggro  [159–164]
- 归属：winona_catapult

### ShouldKeepTarget  [149–153]
- 归属：winona_catapult

### fn  [1010–1180]
- 归属：winona_catapult

### itemfn  [1239–1281]
- 归属：winona_catapult_item

### placer_postinit_fn  [1208–1223]
- 归属：（未归属）

