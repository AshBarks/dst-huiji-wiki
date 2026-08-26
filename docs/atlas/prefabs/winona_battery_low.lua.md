# `prefabs/winona_battery_low.lua`

- 扫描角色：prefabs/winona_battery_low.lua
- 归属变体（2 个）：winona_battery_low, winona_battery_low_item
## 关联

### 组件
- `components/battery.lua`：winona_battery_low（Direct；line 943）
- `components/burnable.lua`：winona_battery_low, winona_battery_low_item（HelperExpanded；line 960,1119）
- `components/circuitnode.lua`：winona_battery_low（Direct；line 935）
- `components/deployable.lua`：winona_battery_low_item（Direct；line 1101）
- `components/deployhelper.lua`：winona_battery_low（Direct；line 885）
- `components/floater.lua`：winona_battery_low_item（HelperExpanded；line 1086）
- `components/fueled.lua`：winona_battery_low, winona_battery_low_item（Direct；line 911,1106）
- `components/hauntable.lua`：winona_battery_low, winona_battery_low_item（Direct/HelperExpanded；line 959,1116）
- `components/inspectable.lua`：winona_battery_low, winona_battery_low_item（Direct；line 908,1098）
- `components/inventoryitem.lua`：winona_battery_low_item（Direct；line 1099）
- `components/lootdropper.lua`：winona_battery_low（Direct；line 928）
- `components/placer.lua`：winona_battery_low, winona_battery_low_item（HelperExpanded；line 1135）
- `components/portablestructure.lua`：winona_battery_low（Direct；line 905）
- `components/propagator.lua`：winona_battery_low, winona_battery_low_item（HelperExpanded；line 961,1120）
- `components/workable.lua`：winona_battery_low（Direct；line 929）

### 预制体依赖
- `collapse_small`：winona_battery_low（Direct；line 1134）
- `prefabs/winona_battery_low.lua`：winona_battery_low_item（Direct；line 1136）
- `winona_battery_low_item`：winona_battery_low（Direct；line 1134）

### 生成引用
- `collapse_small`：winona_battery_low（Direct；line 381,389）
- `prefabs/winona_battery_low.lua`：winona_battery_low_item（Direct；line 1024）
- `winona_battery_low_item`：winona_battery_low（Direct；line 343）


## 函数

### AdjustLevelsByPriority  [314–326]
- 归属：winona_battery_low, winona_battery_low_item

### ApplyEfficiencyBonus  [39–46]
- 归属：winona_battery_low, winona_battery_low_item

### BroadcastCircuitChanged  [155–159]
- 归属：winona_battery_low, winona_battery_low_item

### CLIENT_PlayFuelSound  [1035–1041]
- 归属：winona_battery_low_item

### CalcActualFuel  [177–198]
- 归属：winona_battery_low

### CalcEfficiencyMult  [35–37]
- 归属：winona_battery_low, winona_battery_low_item

### CalcFuelMultiplier  [579–581]
- 归属：winona_battery_low, winona_battery_low_item

### CalcFuelRateRescale  [29–33]
- 归属：winona_battery_low, winona_battery_low_item

### CanAddFuelItem  [462–467]
- 归属：winona_battery_low, winona_battery_low_item

### CanBeUsedAsBattery  [200–222]
- 归属：winona_battery_low

### ChangeToItem  [342–350]
- 归属：winona_battery_low

### CheckElementalBattery  [334–338]
- 归属：winona_battery_low

### ClearAllFuelLevels  [306–311]
- 归属：winona_battery_low, winona_battery_low_item

### ConfigureSkillTreeUpgrades  [66–84]
- 归属：winona_battery_low, winona_battery_low_item

### ConsumeBatteryAmount  [563–577]
- 归属：winona_battery_low

### CopyAllProperties  [328–332]
- 归属：winona_battery_low, winona_battery_low_item

### DoAddBatteryPower  [90–92]
- 归属：winona_battery_low, winona_battery_low_item

### DoBuiltOrDeployed  [714–756]
- 归属：winona_battery_low, winona_battery_low_item

### GetStatus  [420–432]
- 归属：winona_battery_low

### IsEngineerOnline  [48–64]
- 归属：winona_battery_low

### Item_OnLoad  [1060–1067]
- 归属：winona_battery_low_item

### Item_OnSave  [1055–1058]
- 归属：winona_battery_low_item

### NotifyCircuitChanged  [151–153]
- 归属：winona_battery_low, winona_battery_low_item

### OnAddFuel  [517–532]
- 归属：winona_battery_low

### OnAddFuelAdjustLevels  [470–484]
- 归属：winona_battery_low, winona_battery_low_item

### OnAddFuelItem  [487–515]
- 归属：winona_battery_low

### OnBatteryTask  [94–96]
- 归属：winona_battery_low, winona_battery_low_item

### OnBuilt  [758–760]
- 归属：winona_battery_low

### OnBuilt1  [699–712]
- 归属：winona_battery_low, winona_battery_low_item

### OnBuilt2  [682–697]
- 归属：winona_battery_low, winona_battery_low_item

### OnBuilt3  [657–680]
- 归属：winona_battery_low, winona_battery_low_item

### OnBurnt  [395–411]
- 归属：winona_battery_low

### OnCircuitChanged  [145–149]
- 归属：winona_battery_low

### OnConnectCircuit  [161–166]
- 归属：winona_battery_low

### OnDeploy  [1023–1033]
- 归属：winona_battery_low_item

### OnDeployed  [762–771]
- 归属：winona_battery_low_item

### OnDisconnectCircuit  [168–173]
- 归属：winona_battery_low

### OnDismantle  [413–416]
- 归属：winona_battery_low

### OnEnableHelper  [795–838]
- 归属：winona_battery_low

### OnEntityWake  [266–270]
- 归属：winona_battery_low

### OnFuelEmpty  [457–459]
- 归属：winona_battery_low, winona_battery_low_item

### OnFuelSectionChange  [544–561]
- 归属：winona_battery_low, winona_battery_low_item

### OnHitAnimOver  [352–361]
- 归属：winona_battery_low

### OnInit  [643–646]
- 归属：winona_battery_low

### OnLoad  [613–641]
- 归属：winona_battery_low

### OnLoadPostPass  [648–653]
- 归属：winona_battery_low

### OnSave  [599–611]
- 归属：winona_battery_low

### OnStartHelper  [840–844]
- 归属：winona_battery_low

### OnUpdateFueled  [534–542]
- 归属：winona_battery_low

### OnUpdatePlacerHelper  [777–793]
- 归属：winona_battery_low

### OnUsedIndirectly  [583–597]
- 归属：winona_battery_low

### OnWorkFinished  [376–385]
- 归属：winona_battery_low

### OnWorked  [369–374]
- 归属：winona_battery_low

### OnWorkedBurnt  [387–393]
- 归属：winona_battery_low

### PlayHitAnim  [363–367]
- 归属：winona_battery_low

### RefreshFuelTypeEffects  [272–304]
- 归属：winona_battery_low, winona_battery_low_item

### ResolvePartialChargeMult  [238–241]
- 归属：winona_battery_low

### SERVER_PlayFuelSound  [1043–1053]
- 归属：winona_battery_low_item

### SetFuelEmpty  [434–455]
- 归属：winona_battery_low, winona_battery_low_item

### StartBattery  [98–102]
- 归属：winona_battery_low, winona_battery_low_item

### StartSoundLoop  [251–259]
- 归属：winona_battery_low, winona_battery_low_item

### StopBattery  [104–109]
- 归属：winona_battery_low, winona_battery_low_item

### StopSoundLoop  [261–264]
- 归属：winona_battery_low, winona_battery_low_item

### UpdateCircuitPower  [111–143]
- 归属：winona_battery_low, winona_battery_low_item

### UpdateSoundLoop  [245–249]
- 归属：winona_battery_low, winona_battery_low_item

### UseAsBattery  [224–236]
- 归属：winona_battery_low

### fn  [848–987]
- 归属：winona_battery_low

### itemfn  [1069–1130]
- 归属：winona_battery_low_item

### placer_postinit_fn  [991–1019]
- 归属：（未归属）

