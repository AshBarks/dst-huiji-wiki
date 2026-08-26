# `prefabs/squid.lua`

- 扫描角色：prefabs/squid.lua
- 归属变体（3 个）：squid, squid_ink_player_fx, squideyelight
## 关联

### 组件
- `components/amphibiouscreature.lua`：squid（Direct；line 190）
- `components/burnable.lua`：squid（HelperExpanded；line 277）
- `components/combat.lua`：squid（Direct；line 220）
- `components/debuff.lua`：squid_ink_player_fx（Direct；line 369）
- `components/eater.lua`：squid（Direct；line 249）
- `components/embarker.lua`：squid（Direct；line 185）
- `components/entitytracker.lua`：squid（Direct；line 212）
- `components/fader.lua`：squideyelight（Direct；line 320）
- `components/follower.lua`：squid（Direct；line 211）
- `components/freezable.lua`：squid（HelperExpanded；line 276）
- `components/hauntable.lua`：squid（HelperExpanded；line 275）
- `components/health.lua`：squid（Direct；line 214）
- `components/herdmember.lua`：squid（Direct；line 265）
- `components/inspectable.lua`：squid（Direct；line 247）
- `components/knownlocations.lua`：squid（Direct；line 261）
- `components/locomotor.lua`：squid（Direct；line 178）
- `components/lootdropper.lua`：squid（Direct；line 244）
- `components/oceanfishable.lua`：squid（Direct；line 269）
- `components/sanityaura.lua`：squid（Direct；line 217）
- `components/sleeper.lua`：squid（Direct；line 254）
- `components/spawnfader.lua`：squid（Direct；line 168）
- `components/timer.lua`：squid（Direct；line 263）

### 状态图
- `stategraphs/SGsquid.lua`：squid（Direct；line 183）

### 大脑
- `brains/squidbrain.lua`：squid（Direct；line 209）

### 预制体依赖
- `inksplat`：squid（Direct；line 376）
- `monstermeat`：squid（Direct；line 376）
- `prefabs/lightbulb.lua`：squid（Direct；line 376）
- `prefabs/squidherd.lua`：squid（Direct；line 376）
- `prefabs/wake_small.lua`：squid（Direct；line 376）
- `squid_ink_player_fx`：squid（Direct；line 376）
- `squidcorpse`：squid（Direct；line 376）
- `squideyelight`：squid（Direct；line 376）

### 行为
- `brains/squidbrain.lua`：AttackWall, ChaseAndAttack, DoAction, Wander（prefabs/squid.lua#squid）


## 函数

### KeepTarget  [95–97]
- 归属：squid

### LaunchProjectile  [53–70]
- 归属：squid

### OnAttached  [329–336]
- 归属：squid_ink_player_fx

### OnAttackOther  [110–117]
- 归属：squid

### OnAttacked  [100–108]
- 归属：squid

### OnChangeFollowSymbol  [325–327]
- 归属：（未归属）

### OnDetached  [338–344]
- 归属：squid_ink_player_fx

### OnEntitySleep  [132–133]
- 归属：squid

### OnLoad  [139–141]
- 归属：squid

### OnNewTarget  [84–88]
- 归属：squid

### OnReelingIn  [119–126]
- 归属：squid

### OnSave  [135–137]
- 归属：squid

### ShouldSleep  [76–82]
- 归属：squid

### ShouldWakeUp  [72–74]
- 归属：squid

### fncommon  [143–296]
- 归属：squid

### geteatchance  [128–130]
- 归属：squid

### inkfn  [346–374]
- 归属：squid_ink_player_fx

### retargetfn  [90–93]
- 归属：squid

### squideyelightfn  [298–323]
- 归属：squideyelight

