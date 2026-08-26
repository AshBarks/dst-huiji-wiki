# `prefabs/beefalo.lua`

- 扫描角色：prefabs/beefalo.lua
- 归属变体（2 个）：beefalo, beefalo_carry
## 关联

### 组件
- `components/beard.lua`：beefalo（Direct；line 1135）
- `components/beefalometrics.lua`：beefalo（Direct；line 1264）
- `components/bloomer.lua`：beefalo（Direct；line 1133）
- `components/brushable.lua`：beefalo（Direct；line 1145）
- `components/burnable.lua`：beefalo（HelperExpanded；line 1218）
- `components/colouradder.lua`：beefalo（Direct；line 1266）
- `components/colourtweener.lua`：beefalo（Direct；line 1282）
- `components/combat.lua`：beefalo（Direct；line 1156）
- `components/domesticatable.lua`：beefalo（Direct；line 1215）
- `components/drownable.lua`：beefalo（Direct；line 1265）
- `components/eater.lua`：beefalo（Direct；line 1151）
- `components/follower.lua`：beefalo（Direct；line 1180）
- `components/freezable.lua`：beefalo（HelperExpanded；line 1219）
- `components/hauntable.lua`：beefalo（HelperExpanded；line 1286）
- `components/health.lua`：beefalo（Direct；line 1162）
- `components/herdmember.lua`：beefalo（Direct；line 1244）
- `components/hitchable.lua`：beefalo（Direct；line 1280）
- `components/hunger.lua`：beefalo（Direct；line 1209）
- `components/inspectable.lua`：beefalo（Direct；line 1172）
- `components/knownlocations.lua`：beefalo（Direct；line 1175）
- `components/leader.lua`：beefalo（Direct；line 1179）
- `components/locomotor.lua`：beefalo（Direct；line 1221）
- `components/lootdropper.lua`：beefalo（Direct；line 1169）
- `components/markable_proxy.lua`：beefalo（Direct；line 1284）
- `components/named.lua`：beefalo（Direct；line 1271）
- `components/periodicspawner.lua`：beefalo（Direct；line 1187）
- `components/planardamage.lua`：beefalo（Direct；line 1132）
- `components/rideable.lua`：beefalo（Direct；line 1197）
- `components/saltlicker.lua`：beefalo（Direct；line 1231）
- `components/sanityaura.lua`：beefalo, beefalo_carry（Direct；line 95）
- `components/skinner_beefalo.lua`：beefalo（Direct；line 1268）
- `components/sleeper.lua`：beefalo（Direct；line 1225）
- `components/timer.lua`：beefalo（Direct；line 1230）
- `components/trader.lua`：beefalo（Direct；line 1203）
- `components/uniqueid.lua`：beefalo（Direct；line 1263）
- `components/writeable.lua`：beefalo（Direct；line 1273）

### 状态图
- `stategraphs/SGBeefalo.lua`：beefalo（Direct；line 1310）

### 大脑
- `brains/beefalobrain.lua`：beefalo（Direct；line 1309）

### 预制体依赖
- `beefalo_carry`：beefalo, beefalo_carry（Direct；line 1349,1350）
- `explode_reskin`：beefalo, beefalo_carry（Direct；line 1349,1350）
- `meat`：beefalo, beefalo_carry（Direct；line 1349,1350）
- `prefabs/beefalowool.lua`：beefalo, beefalo_carry（Direct；line 1349,1350）
- `prefabs/carrat.lua`：beefalo, beefalo_carry（Direct；line 1349,1350）
- `prefabs/horn.lua`：beefalo, beefalo_carry（Direct；line 1349,1350）
- `prefabs/poop.lua`：beefalo, beefalo_carry（Direct；line 1349,1350）
- `spawn_fx_medium`：beefalo, beefalo_carry（Direct；line 1349,1350）

### 生成引用
- `explode_reskin`：beefalo, beefalo_carry（Direct；line 416）
- `prefabs/carrat.lua`：beefalo（Direct；line 137）
- `spawn_fx_medium`：beefalo, beefalo_carry（Direct；line 907）

### 行为
- `brains/beefalobrain.lua`：AttackWall, ChaseAndAttack, FaceEntity, Follow, Wander（prefabs/beefalo.lua#beefalo）


## 函数

### ApplyBuildOverrides  [259–290]
- 归属：beefalo

### CalculateBuckDelay  [625–653]
- 归属：beefalo

### CanShareTarget  [343–347]
- 归属：beefalo

### CanShaveTest  [445–460]
- 归属：beefalo

### CanSpawnPoop  [971–981]
- 归属：beefalo

### ClearBuildOverrides  [230–236]
- 归属：beefalo

### CustomOnHaunt  [937–940]
- 归属：beefalo

### Dead_AbleToAcceptTest  [493–495]
- 归属：beefalo

### DoDomestication  [534–540]
- 归属：beefalo

### DoFeral  [549–555]
- 归属：beefalo

### DoRiderSleep  [781–784]
- 归属：beefalo

### DomesticationTriggerFn  [724–728]
- 归属：beefalo

### GetBaseSkin  [613–615]
- 归属：beefalo

### GetDebugString  [983–985]
- 归属：beefalo

### GetStatus  [374–391]
- 归属：beefalo

### KeepTarget  [332–335]
- 归属：beefalo

### MountSleepTest  [914–920]
- 归属：beefalo

### OnAttacked  [349–372]
- 归属：beefalo

### OnBeingRidden  [773–775]
- 归属：beefalo

### OnBrushed  [482–491]
- 归属：beefalo

### OnBuckTime  [655–659]
- 归属：beefalo

### OnDeath  [671–703]
- 归属：beefalo

### OnDomesticated  [530–532]
- 归属：beefalo

### OnDomesticationDelta  [761–763]
- 归属：beefalo

### OnEat  [746–759]
- 归属：beefalo

### OnEnterMood  [292–302]
- 归属：beefalo

### OnFeral  [542–547]
- 归属：beefalo

### OnGetItemFromPlayer  [505–513]
- 归属：beefalo

### OnHairGrowth  [467–474]
- 归属：beefalo

### OnHealthDelta  [765–771]
- 归属：beefalo

### OnHitchTo  [867–873]
- 归属：beefalo

### OnHungerDelta  [736–744]
- 归属：beefalo

### OnInit  [933–935]
- 归属：beefalo

### OnLeaveMood  [304–310]
- 归属：beefalo

### OnNewTarget  [337–341]
- 归属：beefalo

### OnObedienceDelta  [661–669]
- 归属：beefalo

### OnRefuseItem  [515–528]
- 归属：beefalo

### OnRefuseRider  [849–851]
- 归属：beefalo

### OnResetBeard  [431–443]
- 归属：beefalo

### OnRiderChanged  [786–820]
- 归属：beefalo

### OnRiderDoAttack  [777–779]
- 归属：beefalo

### OnRiderSleep  [853–859]
- 归属：beefalo

### OnSaddleChanged  [832–840]
- 归属：beefalo

### OnShaved  [462–465]
- 归属：beefalo

### OnStarving  [730–734]
- 归属：beefalo

### OnUnhitch  [875–880]
- 归属：beefalo

### PoopOnSpawned  [1038–1053]
- 归属：beefalo

### PotentialRiderTest  [822–830]
- 归属：beefalo

### Retarget  [315–330]
- 归属：beefalo

### SetTendency  [565–611]
- 归属：beefalo

### ShouldAcceptItem  [497–503]
- 归属：beefalo

### ShouldBeg  [617–623]
- 归属：beefalo

### ShouldWakeUp  [898–902]
- 归属：beefalo

### ToggleDomesticationDecay  [922–924]
- 归属：beefalo

### UpdateDomestication  [557–563]
- 归属：beefalo

### _OnRefuseRider  [842–847]
- 归属：beefalo

### addcarrat  [130–134]
- 归属：beefalo

### beefalo  [1065–1320]
- 归属：beefalo

### beefalo_carry  [1322–1346]
- 归属：beefalo_carry

### canalterbuild  [110–114]
- 归属：（未归属）

### createcarrat  [136–143]
- 归属：beefalo

### dobeefalounhitch  [861–865]
- 归属：beefalo

### fns.GetIsInMood  [253–256]
- 归属：（未归属）

### fns.GetMoodComponent  [246–251]
- 归属：（未归属）

### fns.OnRevived  [705–722]
- 归属：（未归属）

### fns.OnShadowPoopEnterLimbo  [1027–1036]
- 归属：（未归属）

### fns.OnShadowPoopTimeOver  [995–1025]
- 归属：（未归属）

### fns.ShouldKeepCorpse  [1055–1063]
- 归属：（未归属）

### getbasebuild  [238–244]
- 归属：beefalo

### onclothingchanged  [987–991]
- 归属：beefalo

### onwenthome  [926–931]
- 归属：beefalo

### removecarrat  [104–108]
- 归属：beefalo

### setcarratart  [116–128]
- 归属：beefalo

### testforcarratexit  [145–166]
- 归属：beefalo

