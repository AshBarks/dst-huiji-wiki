# `prefabs/cave_banana_tree.lua`

- 扫描角色：prefabs/cave_banana_tree.lua
- 归属变体（3 个）：cave_banana_burnt, cave_banana_stump, cave_banana_tree
## 关联

### 组件
- `components/burnable.lua`：cave_banana_stump, cave_banana_tree（HelperExpanded；line 164,239）
- `components/hauntable.lua`：cave_banana_burnt（HelperExpanded；line 305）
- `components/inspectable.lua`：cave_banana_burnt, cave_banana_stump, cave_banana_tree（Direct；line 161,232,297）
- `components/lootdropper.lua`：cave_banana_burnt, cave_banana_stump, cave_banana_tree（Direct；line 160,231,298）
- `components/pickable.lua`：cave_banana_tree（Direct；line 146）
- `components/propagator.lua`：cave_banana_stump, cave_banana_tree（HelperExpanded；line 165,240）
- `components/workable.lua`：cave_banana_burnt, cave_banana_stump, cave_banana_tree（Direct；line 154,234,300）

### 预制体依赖
- `cave_banana`：cave_banana_tree（Direct；line 313）
- `cave_banana_burnt`：cave_banana_tree（Direct；line 313）
- `cave_banana_stump`：cave_banana_tree（Direct；line 313）
- `prefabs/ash.lua`：cave_banana_stump（Direct；line 315）
- `prefabs/charcoal.lua`：cave_banana_burnt（Direct；line 314）
- `prefabs/log.lua`：cave_banana_tree（Direct；line 313）
- `prefabs/twigs.lua`：cave_banana_tree（Direct；line 313）


## 函数

### burnt_chopped  [250–257]
- 归属：cave_banana_burnt
- 掉落锚点：254:charcoal

### burnt_fn  [270–311]
- 归属：cave_banana_burnt

### burnt_onload  [263–268]
- 归属：cave_banana_burnt

### burnt_onsave  [259–261]
- 归属：cave_banana_burnt

### makeemptyfn  [46–49]
- 归属：cave_banana_tree

### makefullfn  [33–37]
- 归属：cave_banana_tree

### onpickedfn  [39–44]
- 归属：cave_banana_tree

### onregenfn  [27–31]
- 归属：cave_banana_tree

### setupstump  [51–54]
- 归属：cave_banana_tree

### stump_burnt  [184–187]
- 归属：cave_banana_stump

### stump_dug  [189–192]
- 归属：cave_banana_stump
- 掉落锚点：190:log

### stump_fn  [205–248]
- 归属：cave_banana_stump

### stump_onload  [198–203]
- 归属：cave_banana_stump

### stump_onsave  [194–196]
- 归属：cave_banana_stump

### stump_startburn  [179–182]
- 归属：cave_banana_stump

### tree_burnt  [90–98]
- 归属：cave_banana_tree

### tree_chop  [76–82]
- 归属：cave_banana_tree

### tree_chopped  [56–74]
- 归属：cave_banana_tree
- 掉落锚点：63:log；64:twigs；65:twigs；68:cave_banana

### tree_fn  [118–177]
- 归属：cave_banana_tree

### tree_onload  [107–116]
- 归属：cave_banana_tree

### tree_onsave  [100–105]
- 归属：cave_banana_tree

### tree_startburn  [84–88]
- 归属：cave_banana_tree

