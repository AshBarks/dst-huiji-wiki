# `prefabs/rabbitking.lua`

- 扫描角色：prefabs/rabbitking.lua
- 归属变体（4 个）：rabbitking_aggressive, rabbitking_lucky, rabbitking_passive, rabbitkingminion_bunnyman
## 关联

### 组件
- `components/acidinfusible.lua`：rabbitkingminion_bunnyman（Direct；line 479）
- `components/bloomer.lua`：rabbitkingminion_bunnyman（Direct；line 437）
- `components/burnable.lua`：rabbitking_aggressive, rabbitking_lucky, rabbitking_passive, rabbitkingminion_bunnyman（HelperExpanded；line 72,448）
- `components/colouradder.lua`：rabbitking_aggressive, rabbitking_lucky, rabbitking_passive（Direct；line 50）
- `components/combat.lua`：rabbitking_aggressive, rabbitking_lucky, rabbitking_passive, rabbitkingminion_bunnyman（Direct；line 69,439）
- `components/drownable.lua`：rabbitking_aggressive, rabbitking_lucky, rabbitking_passive, rabbitkingminion_bunnyman（Direct；line 62,435）
- `components/eater.lua`：rabbitking_aggressive, rabbitking_lucky, rabbitking_passive（Direct；line 58）
- `components/follower.lua`：rabbitkingminion_bunnyman（Direct；line 454）
- `components/freezable.lua`：rabbitking_aggressive, rabbitking_lucky, rabbitking_passive, rabbitkingminion_bunnyman（HelperExpanded；line 73,475）
- `components/hauntable.lua`：rabbitkingminion_bunnyman（HelperExpanded；line 483）
- `components/health.lua`：rabbitking_aggressive, rabbitking_lucky, rabbitking_passive, rabbitkingminion_bunnyman（Direct；line 64,457）
- `components/inspectable.lua`：rabbitking_aggressive, rabbitking_lucky, rabbitking_passive, rabbitkingminion_bunnyman（Direct；line 75,477）
- `components/inventoryitem.lua`：rabbitking_lucky（Direct；line 588）
- `components/knownlocations.lua`：rabbitking_aggressive, rabbitking_lucky, rabbitking_passive（Direct；line 61）
- `components/leader.lua`：rabbitking_aggressive（Direct；line 271）
- `components/locomotor.lua`：rabbitking_aggressive, rabbitking_lucky, rabbitking_passive, rabbitkingminion_bunnyman（Direct；line 52,430）
- `components/lootdropper.lua`：rabbitking_aggressive, rabbitking_lucky, rabbitking_passive, rabbitkingminion_bunnyman（Direct；line 67,461）
- `components/named.lua`：rabbitkingminion_bunnyman（Direct；line 450）
- `components/prototyper.lua`：rabbitking_passive（Direct；line 135）
- `components/sanityaura.lua`：rabbitking_aggressive, rabbitking_lucky, rabbitking_passive, rabbitkingminion_bunnyman（Direct；line 130,278,469,580）
- `components/sleeper.lua`：rabbitking_aggressive, rabbitking_lucky, rabbitking_passive, rabbitkingminion_bunnyman（Direct；line 76,472）
- `components/talker.lua`：rabbitkingminion_bunnyman（Direct；line 404）
- `components/timer.lua`：rabbitking_aggressive（Direct；line 274）

### 状态图
- `stategraphs/SGrabbitking.lua`：rabbitking_aggressive, rabbitking_lucky, rabbitking_passive（Direct；line 54）
- `stategraphs/SGrabbitking_bunnyman.lua`：rabbitkingminion_bunnyman（Direct；line 486）

### 大脑
- `brains/rabbitking_bunnymanbrain.lua`：rabbitkingminion_bunnyman（Direct；line 485）
- `brains/rabbitkingbrain.lua`：rabbitking_aggressive, rabbitking_lucky, rabbitking_passive（Direct；line 56）

### 预制体依赖
- `-- shop
    "armor_carrotlure"`：rabbitking_passive（Direct；line 608）
- `meat`：rabbitkingminion_bunnyman（Direct；line 610）
- `monstermeat`：rabbitking_aggressive, rabbitkingminion_bunnyman（Direct；line 609,610）
- `prefabs/beardhair.lua`：rabbitking_aggressive, rabbitkingminion_bunnyman（Direct；line 609,610）
- `prefabs/manrabbit_tail.lua`：rabbitkingminion_bunnyman（Direct；line 610）
- `prefabs/rabbitkinghorn.lua`：rabbitking_passive（Direct；line 608）
- `prefabs/rabbitkingspear.lua`：rabbitking_aggressive（Direct；line 609）
- `rabbithat`：rabbitking_passive（Direct；line 608）
- `rabbitkingcorpse`：rabbitking_aggressive, rabbitking_lucky, rabbitking_passive（Direct；line 608,609,611）
- `rabbitkingminion_bunnyman`：rabbitking_aggressive（Direct；line 609）
- `rabbitkingminion_bunnymancorpse`：rabbitkingminion_bunnyman（Direct；line 610）
- `smallmeat`：rabbitking_lucky, rabbitking_passive（Direct；line 608,611）

### 行为
- `brains/rabbitking_bunnymanbrain.lua`：ChaseAndAttack, ChattyNode, Follow, Panic, Wander（prefabs/rabbitking.lua#rabbitkingminion_bunnyman）
- `brains/rabbitkingbrain.lua`：DoAction, FaceEntity, Leash, RunAway, Wander（prefabs/rabbitking.lua#rabbitking_aggressive, prefabs/rabbitking.lua#rabbitking_lucky, prefabs/rabbitking.lua#rabbitking_passive）


## 函数

### BringMinions_Aggressive  [237–249]
- 归属：rabbitking_aggressive

### CanDropkick_Aggressive  [251–257]
- 归属：rabbitking_aggressive

### CanSummonMinions_Aggressive  [190–196]
- 归属：rabbitking_aggressive

### CheckRabbitKingManager  [7–17]
- 归属：rabbitking_aggressive, rabbitking_lucky, rabbitking_passive

### ConvertLuckyToRabbitKing  [519–544]
- 归属：rabbitking_lucky

### ConvertLuckyToRabbitKing_Bridge  [545–548]
- 归属：rabbitking_lucky

### FindMinionSpawnPos_Aggressive  [200–211]
- 归属：rabbitking_aggressive

### ForceTeleport  [348–352]
- 归属：rabbitkingminion_bunnyman

### ForceTeleport_Safe  [341–347]
- 归属：rabbitkingminion_bunnyman

### KeepTargetFunction_Aggressive  [178–180]
- 归属：rabbitking_aggressive

### NoHoles  [197–199]
- 归属：rabbitking_aggressive

### NormalKeepTargetFn  [324–326]
- 归属：rabbitkingminion_bunnyman

### NormalLeaderRetargetFn  [320–323]
- 归属：rabbitkingminion_bunnyman

### OnActivate_passive  [112–114]
- 归属：rabbitking_passive

### OnDropped_lucky  [553–559]
- 归属：rabbitking_lucky

### OnLoad_bunnyman  [359–367]
- 归属：rabbitkingminion_bunnyman

### OnLoad_lucky  [560–565]
- 归属：rabbitking_lucky

### OnLostFollower_Aggressive  [182–188]
- 归属：rabbitking_aggressive

### OnPutInInventory_lucky  [549–552]
- 归属：rabbitking_lucky

### OnSave_bunnyman  [353–358]
- 归属：rabbitkingminion_bunnyman

### OnTalk_Bunnyman  [317–319]
- 归属：rabbitkingminion_bunnyman

### OnTurnOff_passive  [109–111]
- 归属：rabbitking_passive

### OnTurnOn_passive  [106–108]
- 归属：rabbitking_passive

### RetargetFunction_Aggressive  [169–176]
- 归属：rabbitking_aggressive

### SummonMinions_Aggressive  [219–236]
- 归属：rabbitking_aggressive

### SummonMinions_Aggressive_Visualize  [212–218]
- 归属：rabbitking_aggressive

### battlecry  [332–340]
- 归属：rabbitkingminion_bunnyman

### fn_aggressive  [259–295]
- 归属：rabbitking_aggressive
- 掉落锚点：281:

### fn_bunnyman  [370–493]
- 归属：rabbitkingminion_bunnyman

### fn_common  [18–82]
- 归属：rabbitking_aggressive, rabbitking_lucky, rabbitking_passive

### fn_lucky  [566–605]
- 归属：rabbitking_lucky
- 掉落锚点：585:

### fn_passive  [115–142]
- 归属：rabbitking_passive
- 掉落锚点：133:

### giveupstring  [328–330]
- 归属：rabbitkingminion_bunnyman

