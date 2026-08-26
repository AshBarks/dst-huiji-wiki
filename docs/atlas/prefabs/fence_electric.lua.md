# `prefabs/fence_electric.lua`

- 扫描角色：prefabs/fence_electric.lua
- 归属变体（2 个）：fence_electric, fence_electric_item
## 关联

### 组件
- `components/combat.lua`：fence_electric（Direct；line 193）
- `components/deployable.lua`：fence_electric_item（Direct；line 270）
- `components/electricconnector.lua`：fence_electric（Direct；line 178）
- `components/floater.lua`：fence_electric_item（HelperExpanded；line 256）
- `components/hauntable.lua`：fence_electric, fence_electric_item（HelperExpanded；line 207,274）
- `components/health.lua`：fence_electric（Direct；line 199）
- `components/inspectable.lua`：fence_electric, fence_electric_item（Direct；line 172,267）
- `components/inventoryitem.lua`：fence_electric_item（Direct；line 268）
- `components/lootdropper.lua`：fence_electric（Direct；line 175）
- `components/placer.lua`：fence_electric, fence_electric_item（HelperExpanded；line 318）
- `components/stackable.lua`：fence_electric_item（Direct；line 264）
- `components/workable.lua`：fence_electric（Direct；line 187）

### 状态图
- `stategraphs/SGfence_electric.lua`：fence_electric（Direct；line 185）

### 预制体依赖
- `collapse_small`：fence_electric（Direct；line 316）
- `prefabs/fence_electric.lua`：fence_electric_item（Direct；line 317）
- `prefabs/fence_electric_field.lua`：fence_electric（Direct；line 316）


## 函数

### ClearObstacle  [40–43]
- 归属：（未归属）

### GetStatus  [107–109]
- 归属：fence_electric

### InitializePathFinding  [30–33]
- 归属：fence_electric

### KeepTargetFn  [50–52]
- 归属：fence_electric

### MakeObstacle  [35–38]
- 归属：（未归属）

### OnDeployFence  [224–239]
- 归属：fence_electric_item

### OnElectricallyLinked  [91–96]
- 归属：fence_electric

### OnElectricallyUnlinked  [98–103]
- 归属：fence_electric

### OnHammered  [54–63]
- 归属：fence_electric

### OnHit  [69–89]
- 归属：fence_electric

### OnIsPathFindingDirty  [18–28]
- 归属：fence_electric

### OnRemove  [45–48]
- 归属：fence_electric

### OnWorked  [65–67]
- 归属：fence_electric

### fn  [121–210]
- 归属：fence_electric
- 掉落锚点：176:

### itemfn  [241–277]
- 归属：fence_electric_item

### placer_postinit  [284–314]
- 归属：（未归属）

