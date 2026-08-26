# `prefabs/frog.lua`

- 扫描角色：prefabs/frog.lua
- 归属变体（2 个）：frog, lunarfrog
## 关联

### 组件
- `components/combat.lua`：frog, lunarfrog（Direct；line 171）
- `components/drownable.lua`：frog, lunarfrog（Direct；line 163）
- `components/embarker.lua`：frog, lunarfrog（Direct；line 162）
- `components/freezable.lua`：frog, lunarfrog（HelperExpanded；line 177）
- `components/hauntable.lua`：frog, lunarfrog（HelperExpanded；line 188）
- `components/health.lua`：frog, lunarfrog（Direct；line 169）
- `components/inspectable.lua`：frog, lunarfrog（Direct；line 183）
- `components/knownlocations.lua`：frog, lunarfrog（Direct；line 182）
- `components/locomotor.lua`：frog, lunarfrog（Direct；line 156）
- `components/lootdropper.lua`：frog, lunarfrog（Direct；line 179）
- `components/planardamage.lua`：lunarfrog（Direct；line 244）
- `components/planarentity.lua`：lunarfrog（Direct；line 242）
- `components/sleeper.lua`：frog（Direct；line 202）
- `components/thief.lua`：frog, lunarfrog（Direct；line 175）

### 状态图
- `stategraphs/SGfrog.lua`：frog, lunarfrog（Direct；line 165）

### 预制体依赖
- `frogcorpse`：frog（Direct；line 258）
- `frogsplash`：frog（Direct；line 258）
- `prefabs/froglegs.lua`：frog, lunarfrog（Direct；line 258,259）


## 函数

### OnAttacked  [88–91]
- 归属：frog, lunarfrog

### OnGoingHome  [93–95]
- 归属：frog, lunarfrog

### OnHitOther  [97–119]
- 归属：frog, lunarfrog

### ShouldSleep  [79–86]
- 归属：frog

### commonfn  [121–191]
- 归属：frog, lunarfrog
- 掉落锚点：180:froglegs

### lunar_common_postinit  [213–226]
- 归属：lunarfrog

### lunarfn  [228–255]
- 归属：lunarfrog

### normalfn  [193–211]
- 归属：frog

### retargetfn  [63–77]
- 归属：frog, lunarfrog

