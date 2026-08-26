# `prefabs/ancienttree_fruits.lua`

- 扫描角色：prefabs/ancienttree_fruits.lua
- 归属变体（4 个）：ancientfruit_gem, ancientfruit_nightvision, ancientfruit_nightvision_cooked, nightvision_buff
## 关联

### 组件
- `components/bait.lua`：ancientfruit_gem（Direct；line 285）
- `components/burnable.lua`：ancientfruit_nightvision, ancientfruit_nightvision_cooked（HelperExpanded；line 469,515）
- `components/cookable.lua`：ancientfruit_nightvision（Direct；line 461）
- `components/debuff.lua`：nightvision_buff（Direct；line 643）
- `components/edible.lua`：ancientfruit_gem, ancientfruit_nightvision, ancientfruit_nightvision_cooked（Direct；line 290,446,502）
- `components/floater.lua`：ancientfruit_nightvision, ancientfruit_nightvision_cooked（HelperExpanded；line 426,490）
- `components/hauntable.lua`：ancientfruit_gem, ancientfruit_nightvision, ancientfruit_nightvision_cooked（HelperExpanded；line 314,472,518）
- `components/inspectable.lua`：ancientfruit_gem, ancientfruit_nightvision, ancientfruit_nightvision_cooked（Direct；line 286,442,498）
- `components/inventoryitem.lua`：ancientfruit_gem, ancientfruit_nightvision, ancientfruit_nightvision_cooked（Direct；line 294,443,499）
- `components/lootdropper.lua`：ancientfruit_gem（Direct；line 287）
- `components/perishable.lua`：ancientfruit_nightvision, ancientfruit_nightvision_cooked（Direct；line 453,507）
- `components/propagator.lua`：ancientfruit_nightvision, ancientfruit_nightvision_cooked（HelperExpanded；line 470,516）
- `components/stackable.lua`：ancientfruit_gem, ancientfruit_nightvision, ancientfruit_nightvision_cooked（Direct；line 297,458,512）
- `components/timer.lua`：ancientfruit_gem（Direct；line 301）
- `components/tradable.lua`：ancientfruit_gem, ancientfruit_nightvision, ancientfruit_nightvision_cooked（Direct；line 288,444,500）

### 预制体依赖
- `ancientfruit_nightvision_cooked`：ancientfruit_nightvision, ancientfruit_nightvision_cooked（Direct；line 662,663）
- `nightvision_buff`：ancientfruit_nightvision, ancientfruit_nightvision_cooked（Direct；line 662,663）


## 函数

### GemFruit_OnDestack  [240–246]
- 归属：ancientfruit_gem

### GemFruit_OnEnterLimbo  [122–131]
- 归属：ancientfruit_gem

### GemFruit_OnExitLimbo  [102–120]
- 归属：ancientfruit_gem

### GemFruit_OnLoad  [147–153]
- 归属：ancientfruit_gem

### GemFruit_OnSave  [133–145]
- 归属：ancientfruit_gem

### GemFruit_OnTimerDone  [213–238]
- 归属：ancientfruit_gem

### GemFruit_OnUpdate  [43–100]
- 归属：ancientfruit_gem

### GemFruit_SpawnAndLaunchGems  [171–211]
- 归属：ancientfruit_gem

### GemFruit_SpawnGem  [155–169]
- 归属：ancientfruit_gem

### NightVision_DoBeatingBounce  [355–367]
- 归属：ancientfruit_nightvision

### NightVision_OnEaten  [339–348]
- 归属：ancientfruit_nightvision

### NightVision_OnEntitySleep  [393–405]
- 归属：ancientfruit_nightvision

### NightVision_OnEntityWake  [369–391]
- 归属：ancientfruit_nightvision

### NightVision_PlayBeatingSound  [350–353]
- 归属：ancientfruit_nightvision

### buff_Expire  [559–563]
- 归属：nightvision_buff

### buff_OnAttached  [525–541]
- 归属：nightvision_buff

### buff_OnDetached  [543–557]
- 归属：nightvision_buff

### buff_OnEnabledDirty  [611–619]
- 归属：nightvision_buff

### buff_OnExtended  [565–572]
- 归属：nightvision_buff

### buff_OnLoad  [580–593]
- 归属：nightvision_buff

### buff_OnLongUpdate  [595–609]
- 归属：nightvision_buff

### buff_OnSave  [574–578]
- 归属：nightvision_buff

### cooked_nightvision_fruit_fn  [477–521]
- 归属：ancientfruit_nightvision_cooked

### fn_nightvisionbuff  [621–657]
- 归属：nightvision_buff

### gem_fruit_fn  [248–317]
- 归属：ancientfruit_gem

### nightvision_fruit_fn  [407–475]
- 归属：ancientfruit_nightvision

