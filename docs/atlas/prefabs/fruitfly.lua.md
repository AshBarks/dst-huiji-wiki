# `prefabs/fruitfly.lua`

- 扫描角色：prefabs/fruitfly.lua
- 归属变体（4 个）：friendlyfruitfly, fruitfly, fruitflyfruit, lordfruitfly
## 关联

### 组件
- `components/burnable.lua`：friendlyfruitfly, fruitfly, fruitflyfruit, lordfruitfly（HelperExpanded；line 89,500）
- `components/combat.lua`：friendlyfruitfly, fruitfly, lordfruitfly（Direct；line 232,329,438）
- `components/floater.lua`：friendlyfruitfly, fruitflyfruit（HelperExpanded；line 420,559）
- `components/follower.lua`：friendlyfruitfly, fruitfly（Direct；line 327,433）
- `components/freezable.lua`：friendlyfruitfly, fruitfly, lordfruitfly（HelperExpanded；line 88）
- `components/fuel.lua`：fruitflyfruit（Direct；line 498）
- `components/hauntable.lua`：friendlyfruitfly, fruitfly, fruitflyfruit, lordfruitfly（HelperExpanded；line 93,573）
- `components/health.lua`：friendlyfruitfly, fruitfly, lordfruitfly（Direct；line 240,338,437）
- `components/inspectable.lua`：friendlyfruitfly, fruitfly, fruitflyfruit, lordfruitfly（Direct；line 79,569）
- `components/inventoryitem.lua`：fruitflyfruit（Direct；line 571）
- `components/knownlocations.lua`：lordfruitfly（Direct；line 243）
- `components/leader.lua`：fruitflyfruit, lordfruitfly（Direct；line 230,567）
- `components/locomotor.lua`：friendlyfruitfly, fruitfly, lordfruitfly（Direct；line 84）
- `components/lootdropper.lua`：friendlyfruitfly, fruitfly, lordfruitfly（Direct；line 246,344,446）
- `components/perishable.lua`：fruitflyfruit（Direct；line 474）
- `components/propagator.lua`：fruitflyfruit（HelperExpanded；line 501）
- `components/sanityaura.lua`：friendlyfruitfly, fruitfly, lordfruitfly（Direct；line 254,348,448）
- `components/sleeper.lua`：friendlyfruitfly, fruitfly, lordfruitfly（Direct；line 81）

### 状态图
- `stategraphs/SGfruitfly.lua`：friendlyfruitfly, fruitfly, lordfruitfly（Direct；line 224,355,452）

### 大脑
- `brains/friendlyfruitflybrain.lua`：friendlyfruitfly（Direct；line 451）
- `brains/fruitflybrain.lua`：fruitfly, lordfruitfly（Direct；line 226,354）

### 预制体依赖
- `friendlyfruitfly`：fruitflyfruit（Direct；line 586）
- `fruitflyfruit`：lordfruitfly（Direct；line 583）
- `prefabs/fruitfly.lua`：lordfruitfly（Direct；line 583）

### 行为
- `brains/friendlyfruitflybrain.lua`：FaceEntity, FindFarmPlant, Follow, Wander（prefabs/fruitfly.lua#friendlyfruitfly）
- `brains/fruitflybrain.lua`：ChaseAndAttack, Wander（prefabs/fruitfly.lua#fruitfly, prefabs/fruitfly.lua#lordfruitfly）


## 函数

### CanTargetAndAttack  [272–274]
- 归属：fruitfly

### FriendlyShouldKeepTarget  [381–383]
- 归属：friendlyfruitfly

### FriendlyShouldSleep  [376–379]
- 归属：friendlyfruitfly

### FriendlyShouldWakeUp  [371–374]
- 归属：friendlyfruitfly

### IsTargetedByOther  [174–184]
- 归属：lordfruitfly

### KeepTargetFn  [122–127]
- 归属：lordfruitfly

### LootSetupFunction  [290–293]
- 归属：fruitfly

### LordLootSetupFunction  [110–120]
- 归属：lordfruitfly

### MiniOnAttacked  [284–288]
- 归属：fruitfly

### MiniRetargetFn  [280–282]
- 归属：fruitfly

### NumFruitFliesToSpawn  [163–172]
- 归属：lordfruitfly

### OnAttacked  [136–144]
- 归属：fruitfly, lordfruitfly

### OnDead  [146–148]
- 归属：lordfruitfly

### OnInit  [529–540]
- 归属：fruitflyfruit

### OnLoad  [98–102]
- 归属：fruitfly, lordfruitfly

### OnLoseChild  [469–502]
- 归属：fruitflyfruit

### OnPreLoad  [508–512]
- 归属：fruitflyfruit

### OnSave  [514–516]
- 归属：fruitfly, fruitflyfruit, lordfruitfly

### OnStartFollowing  [389–393]
- 归属：friendlyfruitfly

### OnStopFollowing  [385–387]
- 归属：friendlyfruitfly

### RememberKnownLocation  [150–153]
- 归属：lordfruitfly

### RetargetFn  [131–134]
- 归属：lordfruitfly

### ShouldKeepTarget  [276–278]
- 归属：fruitfly

### ShouldSleep  [155–157]
- 归属：fruitfly, lordfruitfly

### ShouldWake  [159–161]
- 归属：fruitfly, lordfruitfly

### SpawnFriendlyFruitFly  [518–527]
- 归属：fruitflyfruit

### common  [56–76]
- 归属：friendlyfruitfly, fruitfly, lordfruitfly

### common_server  [78–96]
- 归属：friendlyfruitfly, fruitfly, lordfruitfly

### fn  [198–270]
- 归属：lordfruitfly

### friendlyfn  [401–456]
- 归属：friendlyfruitfly

### fruitfn  [542–581]
- 归属：fruitflyfruit

### getstatus  [504–506]
- 归属：fruitflyfruit

### minifn  [301–366]
- 归属：fruitfly

### pickseed  [39–49]
- 归属：lordfruitfly

