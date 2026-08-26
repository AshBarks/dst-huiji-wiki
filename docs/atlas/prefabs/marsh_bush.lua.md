# `prefabs/marsh_bush.lua`

- 扫描角色：prefabs/marsh_bush.lua
- 归属变体（3 个）：burnt_marsh_bush, burnt_marsh_bush_erode, marsh_bush
## 关联

### 组件
- `components/activatable.lua`：burnt_marsh_bush（Direct；line 162）
- `components/burnable.lua`：marsh_bush（HelperExpanded；line 119）
- `components/hauntable.lua`：burnt_marsh_bush, marsh_bush（Direct/HelperExpanded；line 121,159）
- `components/inspectable.lua`：burnt_marsh_bush, marsh_bush（Direct；line 117,158）
- `components/lootdropper.lua`：burnt_marsh_bush, marsh_bush（Direct；line 56,111）
- `components/pickable.lua`：marsh_bush（Direct；line 102）
- `components/propagator.lua`：marsh_bush（HelperExpanded；line 120）
- `components/waxable.lua`：marsh_bush（HelperExpanded；line 123）
- `components/workable.lua`：marsh_bush（Direct；line 112）

### 预制体依赖
- `burnt_marsh_bush_erode`：burnt_marsh_bush（Direct；line 226）
- `dug_marsh_bush`：marsh_bush（Direct；line 225）
- `prefabs/ash.lua`：burnt_marsh_bush（Direct；line 226）
- `prefabs/twigs.lua`：marsh_bush（Direct；line 225）

### 生成引用
- `burnt_marsh_bush_erode`：burnt_marsh_bush（Direct；line 67）


## 函数

### DropAsh  [54–59]
- 归属：burnt_marsh_bush
- 掉落锚点：58:ash

### GetVerb  [128–130]
- 归属：burnt_marsh_bush

### OnActivateBurnt  [61–68]
- 归属：burnt_marsh_bush

### PlayErodeAnim  [169–192]
- 归属：burnt_marsh_bush_erode

### burnt_erode_fn  [194–223]
- 归属：burnt_marsh_bush_erode

### burnt_fn  [132–167]
- 归属：burnt_marsh_bush

### dig_up  [28–34]
- 归属：marsh_bush
- 掉落锚点：30:twigs；32:dug_marsh_bush

### fn  [70–126]
- 归属：marsh_bush

### makeemptyfn  [50–52]
- 归属：marsh_bush

### onpickedfn  [36–43]
- 归属：marsh_bush

### onregenfn  [45–48]
- 归属：marsh_bush

### ontransplantfn  [24–26]
- 归属：marsh_bush

