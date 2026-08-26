# `prefabs/wormlight.lua`

- 扫描角色：prefabs/wormlight.lua
- 归属变体（8 个）：wormlight, wormlight_lesser, wormlight_light, wormlight_light_fx, wormlight_light_fx_greater, wormlight_light_fx_lesser, wormlight_light_greater, wormlight_light_lesser
## 关联

### 组件
- `components/edible.lua`：wormlight, wormlight_lesser（Direct；line 97）
- `components/floater.lua`：wormlight, wormlight_lesser（HelperExpanded；line 85）
- `components/fuel.lua`：wormlight, wormlight_lesser（Direct；line 104）
- `components/inspectable.lua`：wormlight, wormlight_lesser（Direct；line 93）
- `components/inventoryitem.lua`：wormlight, wormlight_lesser（Direct；line 94）
- `components/perishable.lua`：wormlight, wormlight_lesser（Direct；line 99）
- `components/spell.lua`：wormlight_light, wormlight_light_greater, wormlight_light_lesser（Direct；line 309）
- `components/stackable.lua`：wormlight, wormlight_lesser（Direct；line 107）
- `components/tradable.lua`：wormlight, wormlight_lesser（Direct；line 95）
- `components/vasedecoration.lua`：wormlight, wormlight_lesser（Direct；line 96）

### 预制体依赖
- `wormlight_light`：wormlight（Direct；line 426）
- `wormlight_light_fx`：wormlight_light（Direct；line 428）
- `wormlight_light_fx_greater`：wormlight_light_greater（Direct；line 430）
- `wormlight_light_fx_lesser`：wormlight_light_lesser（Direct；line 429）
- `wormlight_light_lesser`：wormlight_lesser（Direct；line 427）


## 函数

### OnLightDirty  [359–364]
- 归属：wormlight_light_fx, wormlight_light_fx_greater, wormlight_light_fx_lesser

### OnOwnerChange  [191–244]
- 归属：wormlight_light, wormlight_light_greater, wormlight_light_lesser

### OnUpdateLight  [342–357]
- 归属：wormlight_light_fx, wormlight_light_fx_greater, wormlight_light_fx_lesser

### create_light  [30–50]
- 归属：wormlight, wormlight_lesser

### forceremove  [252–254]
- 归属：wormlight_light, wormlight_light_greater, wormlight_light_lesser

### greaterlightfn  [336–338]
- 归属：wormlight_light_greater

### greaterlightfxfn  [422–424]
- 归属：wormlight_light_fx_greater

### item_commonfn  [60–115]
- 归属：wormlight, wormlight_lesser

### item_oneaten  [52–54]
- 归属：wormlight

### itemfn  [117–131]
- 归属：wormlight

### lesseritem_oneaten  [56–58]
- 归属：wormlight_lesser

### lesseritemfn  [133–147]
- 归属：wormlight_lesser

### lesserlightfn  [332–334]
- 归属：wormlight_light_lesser

### lesserlightfxfn  [418–420]
- 归属：wormlight_light_fx_lesser

### light_commonfn  [298–326]
- 归属：wormlight_light, wormlight_light_greater, wormlight_light_lesser

### light_onfinish  [278–292]
- 归属：wormlight_light, wormlight_light_greater, wormlight_light_lesser

### light_onremove  [294–296]
- 归属：wormlight_light, wormlight_light_greater, wormlight_light_lesser

### light_ontarget  [246–276]
- 归属：wormlight_light, wormlight_light_greater, wormlight_light_lesser

### light_resume  [166–168]
- 归属：wormlight_light, wormlight_light_greater, wormlight_light_lesser

### light_start  [170–172]
- 归属：wormlight_light, wormlight_light_greater, wormlight_light_lesser

### lightfn  [328–330]
- 归属：wormlight_light

### lightfx_commonfn  [376–412]
- 归属：wormlight_light_fx, wormlight_light_fx_greater, wormlight_light_fx_lesser

### lightfxfn  [414–416]
- 归属：wormlight_light_fx

### popbloom  [182–188]
- 归属：wormlight_light, wormlight_light_greater, wormlight_light_lesser

### pushbloom  [174–180]
- 归属：wormlight_light, wormlight_light_greater, wormlight_light_lesser

### setdead  [371–374]
- 归属：wormlight_light_fx, wormlight_light_fx_greater, wormlight_light_fx_lesser

### setprogress  [366–369]
- 归属：wormlight_light_fx, wormlight_light_fx_greater, wormlight_light_fx_lesser

