# `prefabs/eyeturret.lua`

- 扫描角色：prefabs/eyeturret.lua
- 归属变体（3 个）：eyeturret, eyeturret_base, eyeturret_item
## 关联

### 组件
- `components/combat.lua`：eyeturret（Direct；line 284）
- `components/deployable.lua`：eyeturret_item（Direct；line 206）
- `components/equippable.lua`：eyeturret（Direct；line 146）
- `components/freezable.lua`：eyeturret（HelperExpanded；line 294）
- `components/hauntable.lua`：eyeturret, eyeturret_item（HelperExpanded；line 201,296）
- `components/health.lua`：eyeturret（Direct；line 280）
- `components/inspectable.lua`：eyeturret, eyeturret_item（Direct；line 197,304）
- `components/inventory.lua`：eyeturret（Direct；line 298）
- `components/inventoryitem.lua`：eyeturret, eyeturret_item（Direct；line 143,198）
- `components/lootdropper.lua`：eyeturret（Direct；line 306）
- `components/placer.lua`：eyeturret, eyeturret_base, eyeturret_item（HelperExpanded；line 358）
- `components/sanityaura.lua`：eyeturret（Direct；line 301）
- `components/weapon.lua`：eyeturret（Direct；line 139）

### 状态图
- `stategraphs/SGeyeturret.lua`：eyeturret（Direct；line 312）

### 预制体依赖
- `eyeturret_base`：eyeturret, eyeturret_item（Direct；line 356,357）
- `prefabs/eye_charge.lua`：eyeturret, eyeturret_item（Direct；line 356,357）


## 函数

### EquipWeapon  [134–150]
- 归属：eyeturret

### FixupSkins  [214–222]
- 归属：eyeturret

### OnAttacked  [126–132]
- 归属：eyeturret

### OnEntityReplicated  [323–328]
- 归属：eyeturret_base

### OnLightDirty  [63–68]
- 归属：eyeturret

### OnUpdateLight  [30–61]
- 归属：eyeturret

### PlacerPostInit  [352–354]
- 归属：（未归属）

### ShareTargetFn  [115–117]
- 归属：eyeturret

### ShouldAggro  [119–124]
- 归属：eyeturret

### basefn  [330–350]
- 归属：eyeturret_base

### fn  [223–316]
- 归属：eyeturret

### itemfn  [176–212]
- 归属：eyeturret_item

### ondeploy  [152–164]
- 归属：eyeturret_item

### retargetfn  [77–108]
- 归属：eyeturret

### shouldKeepTarget  [110–113]
- 归属：eyeturret

### syncanim  [166–169]
- 归属：eyeturret

### syncanimpush  [171–174]
- 归属：eyeturret

### triggerlight  [70–73]
- 归属：eyeturret

