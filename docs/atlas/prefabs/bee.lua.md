# `prefabs/bee.lua`

- 扫描角色：prefabs/bee.lua
- 归属变体（2 个）：bee, killerbee
## 关联

### 组件
- `components/burnable.lua`：bee, killerbee（HelperExpanded；line 205）
- `components/combat.lua`：bee, killerbee（Direct；line 211）
- `components/eater.lua`：bee, killerbee（HelperExpanded；line 236）
- `components/floater.lua`：bee, killerbee（HelperExpanded；line 168）
- `components/freezable.lua`：bee, killerbee（HelperExpanded；line 206）
- `components/hauntable.lua`：bee, killerbee（HelperExpanded；line 282,310）
- `components/health.lua`：bee, killerbee（Direct；line 210）
- `components/inspectable.lua`：bee, killerbee（Direct；line 227）
- `components/inventoryitem.lua`：bee, killerbee（Direct；line 184）
- `components/knownlocations.lua`：bee, killerbee（Direct；line 223）
- `components/locomotor.lua`：bee, killerbee（Direct；line 178）
- `components/lootdropper.lua`：bee, killerbee（Direct；line 194）
- `components/pollinator.lua`：bee（Direct；line 278）
- `components/sleeper.lua`：bee, killerbee（Direct；line 219）
- `components/stackable.lua`：bee, killerbee（Direct；line 183）
- `components/tradable.lua`：bee, killerbee（Direct；line 231）
- `components/workable.lua`：bee, killerbee（Direct；line 200）

### 状态图
- `stategraphs/SGbee.lua`：bee, killerbee（Direct；line 181）

### 大脑
- `brains/beebrain.lua`：bee（Direct；line 279）
- `brains/killerbeebrain.lua`：killerbee（Direct；line 307）

### 预制体依赖
- `beecorpse`：bee, killerbee（Direct；line 318,319）
- `prefabs/honey.lua`：bee, killerbee（Direct；line 318,319）
- `prefabs/stinger.lua`：bee, killerbee（Direct；line 318,319）

### 行为
- `brains/beebrain.lua`：ChaseAndAttack, DoAction, FindFlower, Panic, RunAway, Wander（prefabs/bee.lua#bee）
- `brains/killerbeebrain.lua`：ChaseAndAttack, DoAction, RunAway, Wander（prefabs/bee.lua#killerbee）


## 函数

### EnableBuzz  [88–100]
- 归属：bee, killerbee

### KillerRetarget  [115–124]
- 归属：killerbee

### OnDropped  [60–80]
- 归属：bee, killerbee

### OnPickedUp  [82–86]
- 归属：bee, killerbee

### OnSleep  [108–110]
- 归属：bee, killerbee

### OnSpawnedFromHaunt  [290–294]
- 归属：killerbee

### OnWake  [102–106]
- 归属：bee, killerbee

### OnWorked  [47–54]
- 归属：bee, killerbee

### SpringBeeRetarget  [126–137]
- 归属：bee

### bonus_damage_via_allergy  [56–58]
- 归属：bee, killerbee

### commonfn  [139–245]
- 归属：bee, killerbee

### killerbee  [296–316]
- 归属：killerbee

### worker_OnIsSpring  [250–258]
- 归属：bee

### workerbee  [260–288]
- 归属：bee

