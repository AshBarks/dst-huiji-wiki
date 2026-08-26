# `prefabs/chest_mimic.lua`

- 扫描角色：prefabs/chest_mimic.lua
- 归属变体（3 个）：chest_mimic, chest_mimic_revealed, chest_mimic_ruinsspawn_tracker
## 关联

### 组件
- `components/burnable.lua`：chest_mimic_revealed（HelperExpanded；line 419）
- `components/combat.lua`：chest_mimic_revealed（Direct；line 356）
- `components/container.lua`：chest_mimic（Direct；line 113）
- `components/eater.lua`：chest_mimic_revealed（Direct；line 365）
- `components/entitytracker.lua`：chest_mimic, chest_mimic_revealed（Direct；line 121,371）
- `components/freezable.lua`：chest_mimic_revealed（HelperExpanded；line 418）
- `components/hauntable.lua`：chest_mimic, chest_mimic_revealed（Direct/HelperExpanded；line 124,416）
- `components/health.lua`：chest_mimic_revealed（Direct；line 374）
- `components/inspectable.lua`：chest_mimic, chest_mimic_revealed（Direct；line 128,378）
- `components/inventory.lua`：chest_mimic_revealed（Direct；line 381）
- `components/knownlocations.lua`：chest_mimic_revealed（Direct；line 386）
- `components/locomotor.lua`：chest_mimic_revealed（Direct；line 389）
- `components/lootdropper.lua`：chest_mimic_revealed（Direct；line 393）
- `components/planardamage.lua`：chest_mimic_revealed（Direct；line 398）
- `components/planarentity.lua`：chest_mimic_revealed（Direct；line 402）
- `components/sanityaura.lua`：chest_mimic_revealed（Direct；line 405）
- `components/scenariorunner.lua`：chest_mimic, chest_mimic_revealed, chest_mimic_ruinsspawn_tracker（Direct；line 162,459,525）
- `components/thief.lua`：chest_mimic_revealed（Direct；line 409）
- `components/timer.lua`：chest_mimic_revealed（Direct；line 412）

### 状态图
- `stategraphs/SGchest_mimic.lua`：chest_mimic_revealed（Direct；line 427）

### 大脑
- `brains/chest_mimicbrain.lua`：chest_mimic_revealed（Direct；line 428）

### 预制体依赖
- `chest_mimic_revealed`：chest_mimic（Direct；line 547）
- `chest_mimic_ruinsspawn_tracker`：chest_mimic（Direct；line 547）
- `prefabs/shadowheart_infused.lua`：chest_mimic（Direct；line 547）
- `slingshot_band_mimic`：chest_mimic（Direct；line 547）


## 函数

### KeepTargetFn  [236–238]
- 归属：chest_mimic_revealed

### OnHitOther  [240–244]
- 归属：chest_mimic_revealed

### OnRevealedAttacked  [247–258]
- 归属：chest_mimic_revealed

### OnRevealedDeath  [281–286]
- 归属：chest_mimic_revealed

### RetargetFn  [224–234]
- 归属：chest_mimic_revealed

### TrackerOnLoad  [489–493]
- 归属：chest_mimic_ruinsspawn_tracker

### TrackerOnSave  [483–487]
- 归属：chest_mimic_ruinsspawn_tracker

### TryTransformBack  [267–279]
- 归属：chest_mimic_revealed

### create_tracker_at_my_feet  [76–84]
- 归属：chest_mimic

### do_transform  [30–51]
- 归属：chest_mimic

### find_morphable  [295–295]
- 归属：chest_mimic_revealed

### fn  [87–186]
- 归属：chest_mimic

### initiate_transform  [53–57]
- 归属：chest_mimic

### loot_setup_fn  [297–309]
- 归属：chest_mimic_revealed
- 掉落锚点：306:

### onclose  [67–73]
- 归属：chest_mimic

### onopen  [60–65]
- 归属：chest_mimic

### revealed_fn  [319–474]
- 归属：chest_mimic_revealed

### ruinstracker_fn  [495–545]
- 归属：chest_mimic_ruinsspawn_tracker

### ruinstracker_on_mimic_died  [479–481]
- 归属：chest_mimic_ruinsspawn_tracker

### transfer_item_to_chest_container  [260–265]
- 归属：chest_mimic_revealed

### transfer_item_to_monster_inventory  [23–28]
- 归属：chest_mimic

