# `prefabs/wx78_scanner.lua`

- 扫描角色：prefabs/wx78_scanner.lua
- 归属变体（4 个）：wx78_scanner, wx78_scanner_fx, wx78_scanner_item, wx78_scanner_succeeded
## 关联

### 组件
- `components/activatable.lua`：wx78_scanner（Direct；line 939）
- `components/cattoy.lua`：wx78_scanner（Direct；line 946）
- `components/deployable.lua`：wx78_scanner_item（Direct；line 339）
- `components/entitytracker.lua`：wx78_scanner, wx78_scanner_item（Direct；line 328,918）
- `components/fader.lua`：wx78_scanner（Direct；line 93）
- `components/floater.lua`：wx78_scanner, wx78_scanner_item, wx78_scanner_succeeded（HelperExpanded；line 306,907,1132）
- `components/follower.lua`：wx78_scanner（Direct；line 921）
- `components/harvestable.lua`：wx78_scanner_succeeded（Direct；line 1147）
- `components/hauntable.lua`：wx78_scanner, wx78_scanner_item, wx78_scanner_succeeded（HelperExpanded；line 346,1001,1165）
- `components/inspectable.lua`：wx78_scanner, wx78_scanner_item, wx78_scanner_succeeded（Direct；line 322,925,1140）
- `components/inventoryitem.lua`：wx78_scanner_item（Direct；line 325）
- `components/locomotor.lua`：wx78_scanner（Direct；line 929）
- `components/maprevealable.lua`：wx78_scanner（Direct；line 914）
- `components/placer.lua`：wx78_scanner, wx78_scanner_fx, wx78_scanner_item, wx78_scanner_succeeded（HelperExpanded；line 1218）
- `components/teacher.lua`：wx78_scanner_succeeded（Direct；line 1143）
- `components/timer.lua`：wx78_scanner, wx78_scanner_succeeded（Direct；line 936,1153）
- `components/updatelooper.lua`：wx78_scanner, wx78_scanner_item（Direct；line 331,951）

### 状态图
- `stategraphs/SGwx78_scanner.lua`：wx78_scanner（Direct；line 993）

### 预制体依赖
- `globalmapiconunderfog`：wx78_scanner（Direct；line 1219）
- `prefabs/scandata.lua`：wx78_scanner_item（Direct；line 1217）
- `prefabs/wx78_scanner.lua`：wx78_scanner_item（Direct；line 1217）
- `wx78_scanner_fx`：wx78_scanner（Direct；line 1219）
- `wx78_scanner_succeeded`：wx78_scanner（Direct；line 1219）

### 生成引用
- `prefabs/scandata.lua`：wx78_scanner（Direct；line 741）
- `prefabs/wx78_scanner.lua`：wx78_scanner_item（Direct；line 223）
- `wx78_scanner_fx`：wx78_scanner（Direct；line 557）
- `wx78_scanner_item`：wx78_scanner_succeeded（Direct；line 1029,1093）


## 函数

### CanDeploy  [180–182]
- 归属：wx78_scanner_item

### CanDoerActivate  [691–694]
- 归属：wx78_scanner

### ComplainAboutCat  [769–773]
- 归属：wx78_scanner

### CreateRingFX  [65–96]
- 归属：wx78_scanner

### DoTurnOff  [757–767]
- 归属：wx78_scanner

### GetActivateVerb  [824–826]
- 归属：wx78_scanner

### GetCreatureRecipeScan  [104–110]
- 归属：wx78_scanner, wx78_scanner_item

### GetScannerPlayerProximityDistance  [42–48]
- 归属：wx78_scanner

### GetScannerScanDistance  [34–40]
- 归属：wx78_scanner

### GetStatus  [709–713]
- 归属：wx78_scanner

### IsInRangeOfPlayer  [715–728]
- 归属：wx78_scanner

### OnActivateFn  [696–707]
- 归属：wx78_scanner

### OnChangedLeader  [202–220]
- 归属：wx78_scanner

### OnEntitySleep  [851–853]
- 归属：wx78_scanner

### OnEntityWake  [847–849]
- 归属：wx78_scanner

### OnPlayedFromCat  [777–786]
- 归属：wx78_scanner

### OnRadarBoostersDirty  [668–672]
- 归属：wx78_scanner

### OnRemove  [840–843]
- 归属：wx78_scanner

### OnReturnedAfterSuccessfulScan  [593–595]
- 归属：wx78_scanner

### OnScanFailed  [451–454]
- 归属：wx78_scanner

### OnScannerDeployed  [222–235]
- 归属：wx78_scanner_item

### OnShowRingFXDirty  [625–666]
- 归属：wx78_scanner

### OnSignalBoosterSkillDirty  [674–678]
- 归属：wx78_scanner

### OnSuccessfulScan  [573–591]
- 归属：wx78_scanner

### OnTargetFound  [456–475]
- 归属：wx78_scanner

### OnTeach  [1016–1018]
- 归属：wx78_scanner_succeeded

### OnUpdateScanCheck  [414–449]
- 归属：wx78_scanner

### RingFX_UpdateRadius  [55–63]
- 归属：wx78_scanner

### SetScanDataLanded  [731–733]
- 归属：wx78_scanner

### SetUpFromScanner  [1051–1056]
- 归属：wx78_scanner_succeeded

### SpawnData  [735–755]
- 归属：wx78_scanner

### StartProximityScan  [366–370]
- 归属：wx78_scanner

### StartScanFX  [553–563]
- 归属：wx78_scanner

### StopAllScanning  [597–620]
- 归属：wx78_scanner

### StopScanFX  [565–571]
- 归属：wx78_scanner

### TryFindTarget  [477–551]
- 归属：wx78_scanner

### UpdateScannerRadarBoosters  [184–200]
- 归属：wx78_scanner

### can_harvest  [1037–1049]
- 归属：wx78_scanner_succeeded

### can_scan_target  [397–412]
- 归属：wx78_scanner

### do_flash_tick  [1070–1081]
- 归属：wx78_scanner_succeeded

### goAway  [1182–1185]
- 归属：wx78_scanner_fx

### hide_top_light  [374–376]
- 归属：wx78_scanner

### image_off  [247–249]
- 归属：wx78_scanner_item

### image_on  [243–245]
- 归属：wx78_scanner_item

### item_loop_fn  [251–281]
- 归属：wx78_scanner_item

### item_owner_fn  [239–241]
- 归属：wx78_scanner_item

### itemfn  [284–349]
- 归属：wx78_scanner_item

### on_harvested  [1020–1035]
- 归属：wx78_scanner_succeeded

### on_scanner_load  [805–820]
- 归属：wx78_scanner

### on_scanner_save  [791–803]
- 归属：wx78_scanner

### on_scanner_timer_done  [683–689]
- 归属：wx78_scanner

### on_succeeded_load  [1064–1068]
- 归属：wx78_scanner_succeeded

### on_succeeded_save  [1058–1062]
- 归属：wx78_scanner_succeeded

### on_succeeded_spawned  [1083–1085]
- 归属：wx78_scanner_succeeded

### on_succeeded_timeout  [1087–1099]
- 归属：wx78_scanner_succeeded

### on_succeeded_timer_done  [1101–1109]
- 归属：wx78_scanner_succeeded

### placer_postinit_fn  [1212–1215]
- 归属：（未归属）

### proximityscan  [112–175]
- 归属：wx78_scanner, wx78_scanner_item

### scanfx_fn  [1187–1210]
- 归属：wx78_scanner_fx

### scanner_loop_fn  [361–364]
- 归属：wx78_scanner

### scanner_owner_fn  [357–359]
- 归属：wx78_scanner

### scannerfn  [858–1006]
- 归属：wx78_scanner

### scannersucceededfn  [1111–1176]
- 归属：wx78_scanner_succeeded

### start_looping_sound  [830–834]
- 归属：wx78_scanner

### stop_looping_sound  [836–838]
- 归属：wx78_scanner

### top_light_flash  [381–395]
- 归属：wx78_scanner

