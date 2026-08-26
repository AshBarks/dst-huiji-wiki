# `prefabs/carnivalgame_shooting.lua`

- 扫描角色：prefabs/carnivalgame_shooting.lua
- 归属变体（4 个）：carnivalgame_shooting_button, carnivalgame_shooting_projectile, carnivalgame_shooting_station, carnivalgame_shooting_target
## 关联

### 组件
- `components/activatable.lua`：carnivalgame_shooting_button（Direct；line 695）
- `components/carnivalgameshooter.lua`：carnivalgame_shooting_station（Direct；line 535）
- `components/complexprojectile.lua`：carnivalgame_shooting_projectile（Direct；line 833）
- `components/groundshadowhandler.lua`：carnivalgame_shooting_projectile（Direct；line 815）
- `components/inspectable.lua`：carnivalgame_shooting_button, carnivalgame_shooting_target（Direct；line 692,752）
- `components/objectspawner.lua`：carnivalgame_shooting_station（Direct；line 532）
- `components/placer.lua`：carnivalgame_shooting_button, carnivalgame_shooting_projectile, carnivalgame_shooting_station, carnivalgame_shooting_target（HelperExpanded；line 849）
- `components/updatelooper.lua`：carnivalgame_shooting_station（Direct；line 515）

### 状态图
- `stategraphs/SGcarnivalgame_shooting_button.lua`：carnivalgame_shooting_button（Direct；line 702）
- `stategraphs/SGcarnivalgame_shooting_target.lua`：carnivalgame_shooting_target（Direct；line 757）

### 预制体依赖
- `carnivalgame_shooting_button`：carnivalgame_shooting_station（Direct；line 846）
- `carnivalgame_shooting_projectile`：carnivalgame_shooting_station（Direct；line 846）
- `carnivalgame_shooting_projectile_fx`：carnivalgame_shooting_projectile（Direct；line 845）
- `carnivalgame_shooting_target`：carnivalgame_shooting_station（Direct；line 846）
- `prefabs/carnival_prizeticket.lua`：carnivalgame_shooting_station（Direct；line 846）

### 生成引用
- `carnivalgame_shooting_button`：carnivalgame_shooting_station（Direct；line 539）
- `carnivalgame_shooting_projectile_fx`：carnivalgame_shooting_projectile（Direct；line 766,779）


## 函数

### Client_OnUpdateAiming  [453–466]
- 归属：carnivalgame_shooting_station

### CreateShootingContollerPlacer  [148–174]
- 归属：（未归属）

### CreateStationFloor  [56–85]
- 归属：carnivalgame_shooting_station

### CreateTargetFloor  [87–116]
- 归属：carnivalgame_shooting_station

### CreateTargetingArrow  [118–146]
- 归属：carnivalgame_shooting_station

### DoEndOfRound  [258–287]
- 归属：carnivalgame_shooting_station

### DoNextRound  [226–256]
- 归属：carnivalgame_shooting_station

### GetActivateVerb  [647–649]
- 归属：carnivalgame_shooting_button, carnivalgame_shooting_target

### OnActivateGame  [199–224]
- 归属：carnivalgame_shooting_station

### OnArrowStateDirty  [468–493]
- 归属：carnivalgame_shooting_station

### OnBuilt  [176–193]
- 归属：carnivalgame_shooting_station

### OnDeactivateGame  [376–407]
- 归属：carnivalgame_shooting_station

### OnRemoveGame  [409–413]
- 归属：carnivalgame_shooting_station

### OnShotHit  [420–430]
- 归属：carnivalgame_shooting_station

### OnStartPlaying  [299–311]
- 归属：carnivalgame_shooting_station

### OnStopPlaying  [338–374]
- 归属：carnivalgame_shooting_station

### OnUpdateGame  [313–315]
- 归属：carnivalgame_shooting_station

### RemoveGameItems  [317–318]
- 归属：carnivalgame_shooting_station

### Server_OnUpdateAiming  [289–297]
- 归属：carnivalgame_shooting_station

### SpawnRewards  [326–336]
- 归属：carnivalgame_shooting_station

### button_GetStatus  [643–645]
- 归属：carnivalgame_shooting_button

### button_OnPress  [655–659]
- 归属：carnivalgame_shooting_button

### button_fn  [661–707]
- 归属：carnivalgame_shooting_button

### create_target_points  [43–51]
- 归属：（未归属）

### createplacertarget  [607–625]
- 归属：（未归属）

### displaynamefn_button  [651–653]
- 归属：carnivalgame_shooting_button

### displaynamefn_target  [716–718]
- 归属：carnivalgame_shooting_target

### on_target_hit  [433–447]
- 归属：carnivalgame_shooting_station

### onthrown  [770–772]
- 归属：carnivalgame_shooting_projectile

### placerdecor  [627–638]
- 归属：（未归属）

### projectile_OnHit  [774–781]
- 归属：carnivalgame_shooting_projectile

### projectilefn  [783–842]
- 归属：carnivalgame_shooting_projectile

### self_destruct  [765–768]
- 归属：carnivalgame_shooting_projectile

### spawnticket  [320–324]
- 归属：carnivalgame_shooting_station
- 掉落锚点：321:carnival_prizeticket

### station_NewObject  [432–450]
- 归属：carnivalgame_shooting_station

### station_common_postinit  [495–523]
- 归属：carnivalgame_shooting_station

### station_fn  [578–580]
- 归属：carnivalgame_shooting_station

### station_master_postinit  [525–576]
- 归属：carnivalgame_shooting_station

### target_GetStatus  [712–714]
- 归属：carnivalgame_shooting_target

### target_turnon  [195–197]
- 归属：carnivalgame_shooting_station

### targetfn  [720–760]
- 归属：carnivalgame_shooting_target

### turnoff_target  [352–354]
- 归属：carnivalgame_shooting_station

