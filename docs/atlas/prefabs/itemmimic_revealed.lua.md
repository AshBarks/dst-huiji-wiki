# `prefabs/itemmimic_revealed.lua`

- 扫描角色：prefabs/itemmimic_revealed.lua
- 归属变体（2 个）：itemmimic_revealed, itemmimic_revealed_shadow
## 关联

### 组件
- `components/health.lua`：itemmimic_revealed（Direct；line 141）
- `components/knownlocations.lua`：itemmimic_revealed（Direct；line 169）
- `components/locomotor.lua`：itemmimic_revealed（Direct；line 145）
- `components/lootdropper.lua`：itemmimic_revealed（Direct；line 151）
- `components/playerprox.lua`：itemmimic_revealed（Direct；line 155）
- `components/sanityaura.lua`：itemmimic_revealed（Direct；line 159）
- `components/timer.lua`：itemmimic_revealed（Direct；line 163）

### 状态图
- `stategraphs/SGitemmimic_revealed.lua`：itemmimic_revealed（Direct；line 179）

### 大脑
- `brains/itemmimic_revealedbrain.lua`：itemmimic_revealed（Direct；line 180）

### 预制体依赖
- `itemmimic_puff`：itemmimic_revealed（Direct；line 287）
- `itemmimic_revealed_shadow`：itemmimic_revealed（Direct；line 287）
- `prefabs/nightmarefuel.lua`：itemmimic_revealed（Direct；line 287）

### 行为
- `brains/itemmimic_revealedbrain.lua`：RunAway（prefabs/itemmimic_revealed.lua#itemmimic_revealed）


## 函数

### DisperseFromBeingSteppedOn  [33–39]
- 归属：itemmimic_revealed

### GetNoLoot  [72–74]
- 归属：itemmimic_revealed

### InitializeShadowEnvelopes  [204–224]
- 归属：itemmimic_revealed_shadow

### OnLoad  [76–80]
- 归属：itemmimic_revealed

### OnSave  [82–84]
- 归属：itemmimic_revealed

### SetNoLoot  [63–70]
- 归属：itemmimic_revealed
- 掉落锚点：66:；68:

### fn  [86–189]
- 归属：itemmimic_revealed
- 掉落锚点：152:

### on_death  [47–49]
- 归属：itemmimic_revealed

### on_eye_down  [28–31]
- 归属：itemmimic_revealed

### on_eye_up  [19–26]
- 归属：itemmimic_revealed

### on_jump_spawn  [51–55]
- 归属：itemmimic_revealed

### on_timer_done  [57–61]
- 归属：itemmimic_revealed

### shadow_fn  [229–285]
- 归属：itemmimic_revealed_shadow

### toggle_tail  [41–45]
- 归属：itemmimic_revealed

