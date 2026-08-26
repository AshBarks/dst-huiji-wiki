# `prefabs/spider.lua`

- 扫描角色：prefabs/spider.lua
- 归属变体（8 个）：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water
## 关联

### 组件
- `components/acidinfusible.lua`：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water（Direct；line 715）
- `components/amphibiouscreature.lua`：spider_water（Direct；line 1050）
- `components/burnable.lua`：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water（HelperExpanded；line 652）
- `components/combat.lua`：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water（Direct；line 658）
- `components/drownable.lua`：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water（Direct；line 640）
- `components/eater.lua`：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water（Direct/HelperExpanded；line 680,721）
- `components/embarker.lua`：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water（Direct；line 639）
- `components/equippable.lua`：spider_spitter（HelperExpanded；line 867）
- `components/follower.lua`：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water（Direct；line 663）
- `components/freezable.lua`：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water（HelperExpanded；line 653）
- `components/halloweenmoonmutable.lua`：spider, spider_dropper, spider_healer, spider_hider, spider_spitter, spider_warrior（Direct；line 776,804,833,869,899,986）
- `components/hauntable.lua`：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water（HelperExpanded；line 722）
- `components/health.lua`：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water（Direct；line 657）
- `components/inspectable.lua`：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water（Direct；line 688）
- `components/inventory.lua`：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water（Direct；line 692）
- `components/inventoryitem.lua`：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water（Direct/HelperExpanded；line 702,867）
- `components/knownlocations.lua`：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water（Direct；line 676）
- `components/locomotor.lua`：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water（Direct；line 632）
- `components/lootdropper.lua`：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water（Direct；line 644）
- `components/sanityaura.lua`：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water（Direct；line 710）
- `components/sleeper.lua`：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water（Direct；line 669）
- `components/spawnfader.lua`：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water（Direct；line 620）
- `components/timer.lua`：spider_water（Direct；line 1056）
- `components/trader.lua`：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water（Direct；line 693）
- `components/weapon.lua`：spider_spitter（HelperExpanded；line 867）

### 状态图
- `stategraphs/SGspider.lua`：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water（Direct；line 642）

### 预制体依赖
- `monstermeat`：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water（Direct；line 1087,1088,1089,1090,1091,1092,1093,1094）
- `prefabs/moonspider_spike.lua`：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water（Direct；line 1087,1088,1089,1090,1091,1092,1093,1094）
- `prefabs/silk.lua`：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water（Direct；line 1087,1088,1089,1090,1091,1092,1093,1094）
- `prefabs/spider_web_spit.lua`：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water（Direct；line 1087,1088,1089,1090,1091,1092,1093,1094）
- `prefabs/spidergland.lua`：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water（Direct；line 1087,1088,1089,1090,1091,1092,1093,1094）
- `spider_heal_fx`：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water（Direct；line 1087,1088,1089,1090,1091,1092,1093,1094）
- `spider_heal_ground_fx`：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water（Direct；line 1087,1088,1089,1090,1091,1092,1093,1094）
- `spider_heal_target_fx`：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water（Direct；line 1087,1088,1089,1090,1091,1092,1093,1094）
- `spider_mutate_fx`：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water（Direct；line 1087,1088,1089,1090,1091,1092,1093,1094）
- `spider_web_spit_acidinfused`：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water（Direct；line 1087,1088,1089,1090,1091,1092,1093,1094）
- `spidercorpse`：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water（Direct；line 1087,1088,1089,1090,1091,1092,1093,1094）

### 生成引用
- `prefabs/moonspider_spike.lua`：spider_moon（Direct；line 458）


## 函数

### BasicWakeCheck  [243–251]
- 归属：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water

### CalcSanityAura  [390–397]
- 归属：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water

### DoHeal  [478–508]
- 归属：spider_healer

### DoReturn  [264–272]
- 归属：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water

### DoSpikeAttack  [439–466]
- 归属：spider_moon

### FindTarget  [210–224]
- 归属：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water

### GetOtherSpiders  [104–118]
- 归属：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water

### HalloweenMoonMutate  [399–408]
- 归属：spider, spider_dropper, spider_healer, spider_hider, spider_spitter, spider_warrior

### IsHost  [302–304]
- 归属：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water

### IsSpiderAlly  [198–206]
- 归属：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water

### LoadCorpseData  [915–933]
- 归属：spider_moon

### MakeWeapon  [411–435]
- 归属：（未归属）

### NormalRetarget  [226–228]
- 归属：spider, spider_healer

### OnAttacked  [306–330]
- 归属：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water

### OnChangedLeader  [550–552]
- 归属：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water

### OnDropped  [372–378]
- 归属：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water

### OnEat  [366–370]
- 归属：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water

### OnEnterWater  [1017–1022]
- 归属：spider_water

### OnEntitySleep  [282–286]
- 归属：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water

### OnExitWater  [1024–1030]
- 归属：spider_water

### OnGetItemFromPlayer  [120–189]
- 归属：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water

### OnGoToSleep  [380–382]
- 归属：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water

### OnIsCaveDay  [274–280]
- 归属：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water

### OnPickup  [510–516]
- 归属：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water

### OnRefuseItem  [191–196]
- 归属：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water

### OnStartLeashing  [340–350]
- 归属：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water

### OnStopLeashing  [352–360]
- 归属：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water

### OnTrapped  [362–364]
- 归属：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water

### OnWakeUp  [384–388]
- 归属：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water

### SaveCorpseData  [554–568]
- 归属：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water

### SetHappyFace  [332–338]
- 归属：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water

### ShouldAcceptItem  [88–100]
- 归属：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water

### ShouldSleep  [253–255]
- 归属：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water

### ShouldWake  [257–262]
- 归属：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water

### SoundPath  [534–548]
- 归属：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water

### SpawnHealFx  [468–476]
- 归属：spider_healer

### Spitter_OnAcidInfuse  [518–524]
- 归属：spider_spitter

### Spitter_OnAcidUninfuse  [526–532]
- 归属：spider_spitter

### SummonFriends  [290–300]
- 归属：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water

### WarriorRetarget  [230–232]
- 归属：spider_dropper, spider_hider, spider_moon, spider_spitter, spider_warrior

### WaterRetarget  [1032–1039]
- 归属：spider_water

### WaterSpider_SetHappyFace  [998–1006]
- 归属：（未归属）

### create_common  [572–754]
- 归属：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water

### create_dropper  [878–906]
- 归属：spider_dropper

### create_healer  [966–995]
- 归属：spider_healer

### create_hider  [813–840]
- 归属：spider_hider

### create_moon  [935–964]
- 归属：spider_moon

### create_spider  [756–781]
- 归属：spider

### create_spitter  [842–876]
- 归属：spider_spitter

### create_warrior  [783–811]
- 归属：spider_warrior

### create_water  [1041–1084]
- 归属：spider_water

### keeptargetfn  [234–241]
- 归属：spider, spider_dropper, spider_healer, spider_hider, spider_moon, spider_spitter, spider_warrior, spider_water

### spider_moon_common_init  [908–913]
- 归属：spider_moon

