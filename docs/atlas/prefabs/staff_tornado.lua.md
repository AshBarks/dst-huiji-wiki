# `prefabs/staff_tornado.lua`

- 扫描角色：prefabs/staff_tornado.lua
- 归属变体（2 个）：staff_tornado, tornado
## 关联

### 组件
- `components/equippable.lua`：staff_tornado（Direct；line 101）
- `components/finiteuses.lua`：staff_tornado（Direct；line 92）
- `components/floater.lua`：staff_tornado（HelperExpanded；line 80）
- `components/hauntable.lua`：staff_tornado（HelperExpanded；line 113）
- `components/inspectable.lua`：staff_tornado（Direct；line 97）
- `components/inventoryitem.lua`：staff_tornado（Direct；line 99）
- `components/knownlocations.lua`：tornado（Direct；line 157）
- `components/locomotor.lua`：tornado（Direct；line 159）
- `components/spellcaster.lua`：staff_tornado（Direct；line 105）

### 状态图
- `stategraphs/SGtornado.lua`：tornado（Direct；line 163）

### 大脑
- `brains/tornadobrain.lua`：tornado（Direct；line 164）

### 预制体依赖
- `tornado`：staff_tornado（Direct；line 175）

### 行为
- `brains/tornadobrain.lua`：Leash, Wander（prefabs/staff_tornado.lua#tornado）


## 函数

### SetDuration  [125–130]
- 归属：tornado

### getspawnlocation  [13–17]
- 归属：staff_tornado

### onequip  [34–44]
- 归属：staff_tornado

### ontornadolifetime  [120–123]
- 归属：tornado

### onunequip  [46–53]
- 归属：staff_tornado

### spawntornado  [19–32]
- 归属：staff_tornado

### staff_fn  [55–116]
- 归属：staff_tornado

### tornado_fn  [132–173]
- 归属：tornado

