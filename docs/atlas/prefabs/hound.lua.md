# `prefabs/hound.lua`

- 扫描角色：prefabs/hound.lua
- 归属变体（8 个）：clayhound, firehound, hedgehound, hound, houndfire, icehound, moonhound, mutatedhound
## 关联

### 组件
- `components/amphibiouscreature.lua`：clayhound, firehound, hedgehound, hound, icehound, moonhound, mutatedhound（Direct；line 530）
- `components/burnable.lua`：hedgehound, hound, houndfire, icehound, moonhound, mutatedhound（HelperExpanded；line 620,697,739,820,846,900）
- `components/combat.lua`：clayhound, firehound, hedgehound, hound, icehound, moonhound, mutatedhound（Direct；line 551）
- `components/eater.lua`：clayhound, firehound, hedgehound, hound, icehound, moonhound, mutatedhound（Direct；line 573）
- `components/embarker.lua`：clayhound, firehound, hedgehound, hound, icehound, moonhound, mutatedhound（Direct；line 524）
- `components/entitytracker.lua`：clayhound, firehound, hedgehound, hound, icehound, moonhound, mutatedhound（Direct；line 543）
- `components/follower.lua`：clayhound, firehound, hedgehound, hound, icehound, moonhound, mutatedhound（Direct；line 540）
- `components/freezable.lua`：clayhound, firehound, hedgehound, hound, icehound, moonhound, mutatedhound（HelperExpanded；line 619,652,673,738,779,819,899）
- `components/halloweenmoonmutable.lua`：hound（Direct；line 622）
- `components/hauntable.lua`：clayhound, firehound, hedgehound, hound, icehound, moonhound, mutatedhound（Direct/HelperExpanded；line 570,586,590）
- `components/health.lua`：clayhound, firehound, hedgehound, hound, icehound, moonhound, mutatedhound（Direct；line 545）
- `components/inspectable.lua`：clayhound, firehound, hedgehound, hound, icehound, moonhound, mutatedhound（Direct；line 562）
- `components/locomotor.lua`：clayhound, firehound, hedgehound, hound, icehound, moonhound, mutatedhound（Direct；line 517）
- `components/lootdropper.lua`：clayhound, firehound, hedgehound, hound, icehound, moonhound, mutatedhound（Direct；line 559）
- `components/propagator.lua`：houndfire（HelperExpanded；line 847）
- `components/sanityaura.lua`：clayhound, firehound, hedgehound, hound, icehound, moonhound, mutatedhound（Direct；line 548）
- `components/sleeper.lua`：clayhound, firehound, hedgehound, hound, icehound, moonhound, mutatedhound（Direct；line 578）
- `components/spawnfader.lua`：clayhound, firehound, hedgehound, hound, icehound, moonhound, mutatedhound（Direct；line 504）

### 状态图
- `stategraphs/SGhound.lua`：clayhound, firehound, hedgehound, hound, icehound, moonhound, mutatedhound（Direct；line 520）

### 大脑
- `brains/houndbrain.lua`：clayhound, firehound, hedgehound, hound, icehound, moonhound, mutatedhound（Direct/Folded；line 538）
- `brains/moonbeastbrain.lua`：moonhound（Folded；line 538）

### 预制体依赖
- `bluegem`：firehound, hedgehound, hound, houndfire, icehound, mutatedhound（Direct；line 920,921,922,925,926,928）
- `houndcorpse`：firehound, hedgehound, hound, houndfire, icehound, mutatedhound（Direct；line 920,921,922,925,926,928）
- `monstermeat`：firehound, hedgehound, hound, houndfire, icehound, mutatedhound（Direct；line 920,921,922,925,926,928）
- `prefabs/eyeflame.lua`：clayhound（Direct；line 924）
- `prefabs/houndstooth.lua`：clayhound, firehound, hedgehound, hound, houndfire, icehound, mutatedhound（Direct；line 920,921,922,924,925,926,928）
- `redgem`：firehound, hedgehound, hound, houndfire, icehound, mutatedhound（Direct；line 920,921,922,925,926,928）
- `redpouch`：clayhound（Direct；line 924）
- `splash_green`：firehound, hedgehound, hound, houndfire, icehound, mutatedhound（Direct；line 920,921,922,925,926,928）

### 行为
- `brains/houndbrain.lua`：AttackWall, ChaseAndAttack, DoAction, FaceEntity, Leash, StandStill, Wander（prefabs/hound.lua#clayhound, prefabs/hound.lua#firehound, prefabs/hound.lua#hedgehound, prefabs/hound.lua#hound, prefabs/hound.lua#icehound, prefabs/hound.lua#moonhound, prefabs/hound.lua#mutatedhound）
- `brains/moonbeastbrain.lua`：AttackWall, ChaseAndAttack, Leash, StandStill（prefabs/hound.lua#moonhound）


## 函数

### DoFireExplosion  [631–641]
- 归属：firehound
- 掉落锚点：639:houndfire

### DoIceExplosion  [666–686]
- 归属：icehound

### DoReturn  [269–280]
- 归属：clayhound, firehound, hedgehound, hound, icehound, moonhound, mutatedhound

### GetReturnPos  [262–267]
- 归属：clayhound, firehound, hedgehound, hound, icehound, moonhound, mutatedhound

### GetStatus  [339–342]
- 归属：clayhound, firehound, hedgehound, hound, icehound, moonhound, mutatedhound

### IsNearMoonBase  [217–220]
- 归属：moonhound

### IsValidTarget  [175–183]
- 归属：clayhound, firehound, hedgehound, hound, icehound, moonhound, mutatedhound

### KeepTarget  [202–215]
- 归属：clayhound, firehound, hedgehound, hound, icehound, moonhound, mutatedhound

### LoadCorpseData  [791–804]
- 归属：mutatedhound

### OnAttackOther  [253–260]
- 归属：clayhound, firehound, hedgehound, hound, icehound, moonhound, mutatedhound

### OnAttacked  [243–251]
- 归属：clayhound, firehound, hedgehound, hound, icehound, moonhound, mutatedhound

### OnChangedLeader  [432–445]
- 归属：clayhound, firehound, hedgehound, hound, icehound, moonhound, mutatedhound

### OnClayPreLoad  [760–764]
- 归属：clayhound

### OnClaySave  [756–758]
- 归属：clayhound

### OnClayUpdateOffset  [766–768]
- 归属：clayhound

### OnEnterWater  [302–307]
- 归属：clayhound, firehound, hedgehound, hound, icehound, moonhound, mutatedhound

### OnEntitySleep  [282–287]
- 归属：clayhound, firehound, hedgehound, hound, icehound, moonhound, mutatedhound

### OnExitWater  [309–316]
- 归属：clayhound, firehound, hedgehound, hound, icehound, moonhound, mutatedhound

### OnEyeFlamesDirty  [344–384]
- 归属：clayhound, firehound, hedgehound, hound, icehound, moonhound, mutatedhound

### OnHedgeKilled  [857–863]
- 归属：hedgehound

### OnLoad  [324–337]
- 归属：clayhound, firehound, hedgehound, hound, icehound, moonhound, mutatedhound

### OnMoonPetrify  [710–720]
- 归属：moonhound

### OnMoonTransformed  [722–727]
- 归属：moonhound

### OnNewTarget  [168–172]
- 归属：clayhound, firehound, hedgehound, hound, icehound, moonhound, mutatedhound

### OnSave  [318–322]
- 归属：clayhound, firehound, hedgehound, hound, icehound, moonhound, mutatedhound

### OnSpawnedFromHaunt  [296–300]
- 归属：clayhound, firehound, hedgehound, hound, icehound, moonhound, mutatedhound

### OnStartFollowing  [386–401]
- 归属：clayhound, firehound, hedgehound, hound, icehound, moonhound, mutatedhound

### OnStopDay  [289–294]
- 归属：clayhound, firehound, hedgehound, hound, icehound, moonhound, mutatedhound

### OnStopFollowing  [412–430]
- 归属：clayhound, firehound, hedgehound, hound, icehound, moonhound, mutatedhound

### RestoreLeader  [403–410]
- 归属：clayhound, firehound, hedgehound, hound, icehound, moonhound, mutatedhound

### SaveCorpseData  [447–457]
- 归属：clayhound, firehound, hedgehound, hound, icehound, moonhound, mutatedhound

### ShouldSleep  [160–166]
- 归属：clayhound, firehound, hedgehound, hound, icehound, moonhound, mutatedhound

### ShouldWakeUp  [152–158]
- 归属：clayhound, firehound, hedgehound, hound, icehound, moonhound, mutatedhound

### fnclay  [770–789]
- 归属：clayhound

### fncold  [688–708]
- 归属：icehound

### fncommon  [459–608]
- 归属：clayhound, firehound, hedgehound, hound, icehound, moonhound, mutatedhound

### fndefault  [610–628]
- 归属：hound

### fnfire  [643–664]
- 归属：firehound

### fnfiredrop  [832–855]
- 归属：houndfire

### fnhedge  [886–917]
- 归属：hedgehound

### fnmoon  [729–754]
- 归属：moonhound

### fnmutated  [806–830]
- 归属：mutatedhound

### moon_keeptargetfn  [237–241]
- 归属：moonhound

### moon_retargetfn  [223–235]
- 归属：moonhound

### retargetfn  [185–200]
- 归属：clayhound, firehound, hedgehound, hound, icehound, moonhound, mutatedhound

