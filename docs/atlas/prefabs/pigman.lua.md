# `prefabs/pigman.lua`

- 扫描角色：prefabs/pigman.lua
- 归属变体（3 个）：moonpig, pigguard, pigman
## 关联

### 组件
- `components/bloomer.lua`：moonpig, pigguard, pigman（Direct；line 656）
- `components/burnable.lua`：moonpig, pigguard, pigman（HelperExpanded；line 670）
- `components/combat.lua`：moonpig, pigguard, pigman（Direct；line 667）
- `components/drownable.lua`：pigguard, pigman（Direct；line 754,775）
- `components/eater.lua`：moonpig, pigguard, pigman（Direct；line 659）
- `components/embarker.lua`：pigguard, pigman（Direct；line 753,774）
- `components/entitytracker.lua`：moonpig（Direct；line 828）
- `components/follower.lua`：moonpig, pigguard, pigman（Direct；line 688）
- `components/freezable.lua`：moonpig, pigguard, pigman（HelperExpanded；line 723）
- `components/hauntable.lua`：moonpig, pigguard, pigman（HelperExpanded；line 677）
- `components/health.lua`：moonpig, pigguard, pigman（Direct；line 666）
- `components/inspectable.lua`：moonpig, pigguard, pigman（Direct；line 727）
- `components/inventory.lua`：moonpig, pigguard, pigman（Direct；line 692）
- `components/knownlocations.lua`：moonpig, pigguard, pigman（Direct；line 700）
- `components/locomotor.lua`：moonpig, pigguard, pigman（Direct；line 652）
- `components/lootdropper.lua`：moonpig, pigguard, pigman（Direct；line 696）
- `components/named.lua`：moonpig, pigguard, pigman（Direct；line 672）
- `components/sanityaura.lua`：moonpig, pigguard, pigman（Direct；line 714）
- `components/sleeper.lua`：moonpig, pigguard, pigman（Direct；line 719）
- `components/spawnfader.lua`：moonpig, pigguard, pigman（Direct；line 616）
- `components/talker.lua`：moonpig, pigguard, pigman（Direct；line 621）
- `components/trader.lua`：moonpig, pigguard, pigman（Direct；line 705）
- `components/werebeast.lua`：moonpig, pigguard, pigman（Direct；line 680）

### 状态图
- `stategraphs/SGmoonpig.lua`：moonpig（Direct；line 831）
- `stategraphs/SGpig.lua`：pigguard, pigman（Direct；line 311,422）
- `stategraphs/SGwerepig.lua`：moonpig, pigguard, pigman（Direct；line 513）

### 预制体依赖
- `meat`：pigguard, pigman（Direct；line 859,860）
- `monstermeat`：pigguard, pigman（Direct；line 859,860）
- `prefabs/pigskin.lua`：pigguard, pigman（Direct；line 859,860）
- `prefabs/poop.lua`：pigguard, pigman（Direct；line 859,860）
- `strawhat`：pigguard, pigman（Direct；line 859,860）
- `tophat`：pigguard, pigman（Direct；line 859,860）

### 生成引用
- `prefabs/poop.lua`：moonpig, pigguard, pigman（Direct；line 131）


## 函数

### CalcSanityAura  [47–52]
- 归属：moonpig, pigguard, pigman

### CustomOnHaunt  [566–573]
- 归属：moonpig, pigguard, pigman

### GetPigToken  [38–41]
- 归属：moonpig, pigguard, pigman

### GetStatus  [540–545]
- 归属：moonpig, pigguard, pigman

### GuardKeepTargetFn  [388–406]
- 归属：pigguard

### GuardRetargetFn  [346–386]
- 归属：pigguard

### GuardShouldSleep  [408–410]
- 归属：pigguard

### GuardShouldWake  [412–414]
- 归属：pigguard

### IsGuardPig  [171–173]
- 归属：moonpig, pigguard, pigman

### IsHost  [175–177]
- 归属：moonpig, pigguard, pigman

### IsNearMoonBase  [472–475]
- 归属：moonpig

### IsNonWerePig  [167–169]
- 归属：moonpig, pigguard, pigman

### IsPig  [159–161]
- 归属：moonpig, pigguard, pigman

### IsWerePig  [163–165]
- 归属：moonpig, pigguard, pigman

### MoonpigKeepTargetFn  [493–497]
- 归属：moonpig

### MoonpigRetargetFn  [478–491]
- 归属：moonpig

### NormalKeepTargetFn  [249–253]
- 归属：pigman

### NormalRetargetFn  [217–247]
- 归属：pigman

### NormalShouldSleep  [256–260]
- 归属：pigman

### OnAttacked  [179–201]
- 归属：moonpig, pigguard, pigman

### OnAttackedByDecidRoot  [144–157]
- 归属：moonpig, pigguard, pigman

### OnEat  [128–139]
- 归属：moonpig, pigguard, pigman

### OnGetItemFromPlayer  [72–119]
- 归属：moonpig, pigguard, pigman

### OnItemGet  [271–276]
- 归属：moonpig, pigguard, pigman

### OnItemLose  [278–283]
- 归属：moonpig, pigguard, pigman

### OnLoad  [556–564]
- 归属：moonpig, pigguard, pigman

### OnMoonPetrify  [800–813]
- 归属：moonpig

### OnMoonTransformed  [815–819]
- 归属：moonpig

### OnNewTarget  [203–211]
- 归属：moonpig, pigguard, pigman

### OnRefuseItem  [121–126]
- 归属：moonpig, pigguard, pigman

### OnSave  [551–554]
- 归属：moonpig, pigguard, pigman

### ReplacePigToken  [294–305]
- 归属：moonpig, pigguard, pigman

### SetGuardPig  [418–448]
- 归属：pigguard
- 掉落锚点：440:

### SetNormalPig  [307–338]
- 归属：pigman
- 掉落锚点：326:

### SetWerePig  [509–538]
- 归属：moonpig, pigguard, pigman
- 掉落锚点：526:meat+meat+pigskin

### SetupPigToken  [285–292]
- 归属：pigman

### ShouldAcceptItem  [54–70]
- 归属：moonpig, pigguard, pigman

### SuggestTreeTarget  [264–269]
- 归属：pigman

### WerepigKeepTargetFn  [465–470]
- 归属：moonpig, pigguard, pigman

### WerepigRetargetFn  [452–463]
- 归属：moonpig, pigguard, pigman

### WerepigSleepTest  [499–501]
- 归属：moonpig, pigguard, pigman

### WerepigWakeTest  [503–505]
- 归属：moonpig, pigguard, pigman

### common  [577–740]
- 归属：moonpig, pigguard, pigman

### displaynamefn  [547–549]
- 归属：moonpig, pigguard, pigman

### guard  [765–782]
- 归属：pigguard

### moon  [821–857]
- 归属：moonpig
- 掉落锚点：844:meat+meat+pigskin

### normal  [742–763]
- 归属：pigman

### ontalk  [43–45]
- 归属：moonpig, pigguard, pigman

