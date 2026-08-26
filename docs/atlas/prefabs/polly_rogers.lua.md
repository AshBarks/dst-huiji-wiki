# `prefabs/polly_rogers.lua`

- 扫描角色：prefabs/polly_rogers.lua
- 归属变体（2 个）：polly_rogers, salty_dog
## 关联

### 组件
- `components/amphibiouscreature.lua`：salty_dog（Direct；line 231）
- `components/burnable.lua`：polly_rogers, salty_dog（HelperExpanded；line 117,246）
- `components/combat.lua`：polly_rogers, salty_dog（Direct；line 78）
- `components/counter.lua`：salty_dog（Direct；line 226）
- `components/eater.lua`：polly_rogers, salty_dog（Direct；line 75）
- `components/embarker.lua`：salty_dog（Direct；line 221）
- `components/follower.lua`：polly_rogers, salty_dog（Direct；line 76）
- `components/freezable.lua`：polly_rogers, salty_dog（HelperExpanded；line 118,247）
- `components/health.lua`：polly_rogers, salty_dog（Direct；line 77）
- `components/inspectable.lua`：polly_rogers, salty_dog（Direct；line 80）
- `components/inventory.lua`：polly_rogers, salty_dog（Direct；line 81）
- `components/locomotor.lua`：polly_rogers, salty_dog（Direct；line 69）
- `components/lootdropper.lua`：polly_rogers, salty_dog（Direct；line 79）
- `components/timer.lua`：salty_dog（Direct；line 228）

### 状态图
- `stategraphs/SGpolly_rogers.lua`：polly_rogers（Direct；line 108）
- `stategraphs/SGsalty_dog.lua`：salty_dog（Direct；line 237）

### 大脑
- `brains/pollyrogerbrain.lua`：polly_rogers, salty_dog（Direct；line 109,238）

### 预制体依赖
- `polly_rogerscorpse`：polly_rogers（Direct；line 252）
- `prefabs/saltrock.lua`：salty_dog（Direct；line 253）

### 生成引用
- `prefabs/saltrock.lua`：salty_dog（Direct；line 130,145）

### 行为
- `brains/pollyrogerbrain.lua`：Follow, StandStill, Wander（prefabs/polly_rogers.lua#polly_rogers, prefabs/polly_rogers.lua#salty_dog）


## 函数

### OnEnterWater  [181–187]
- 归属：salty_dog

### OnExitWater  [189–193]
- 归属：salty_dog

### OnLoad_dog  [195–197]
- 归属：salty_dog

### OnTimerDone  [168–179]
- 归属：salty_dog

### ShedAllSalt  [137–151]
- 归属：salty_dog

### ShedSalt  [123–135]
- 归属：salty_dog

### UpdateSaltVisuals  [153–166]
- 归属：salty_dog

### fn_common  [31–85]
- 归属：polly_rogers, salty_dog

### fn_dog  [199–250]
- 归属：salty_dog

### fn_polly  [87–121]
- 归属：polly_rogers

