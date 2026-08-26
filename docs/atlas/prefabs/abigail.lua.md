# `prefabs/abigail.lua`

- 扫描角色：prefabs/abigail.lua
- 归属变体（4 个）：abigail, abigail_murder_buff, abigail_retaliation, abigail_vex_hit
## 关联

### 组件
- `components/aura.lua`：abigail（Direct；line 864）
- `components/combat.lua`：abigail（Direct；line 871）
- `components/damagetypebonus.lua`：abigail（Direct；line 916）
- `components/damagetyperesist.lua`：abigail（Direct；line 915）
- `components/debuff.lua`：abigail_murder_buff（Direct；line 1282）
- `components/debuffable.lua`：abigail（Direct；line 877）
- `components/fader.lua`：abigail（Direct；line 828）
- `components/follower.lua`：abigail（Direct；line 882）
- `components/ghostlyelixirable.lua`：abigail（Direct；line 888）
- `components/health.lua`：abigail（Direct；line 894）
- `components/inspectable.lua`：abigail（Direct；line 901）
- `components/locomotor.lua`：abigail（Direct；line 905）
- `components/planardamage.lua`：abigail（Direct；line 912）
- `components/planardefense.lua`：abigail（Direct；line 919）
- `components/timer.lua`：abigail（Direct；line 922）
- `components/trader.lua`：abigail（Direct；line 925）

### 状态图
- `stategraphs/SGabigail.lua`：abigail（Direct；line 955）

### 大脑
- `brains/abigailbrain.lua`：abigail（Direct；line 954）

### 预制体依赖
- `abigail_attack_fx_ground`：abigail（Direct；line 1292）
- `abigail_attack_shadow_fx`：abigail（Direct；line 1292）
- `abigail_gestalt_hit_fx`：abigail（Direct；line 1292）
- `abigail_retaliation`：abigail（Direct；line 1292）
- `abigail_rising_twinkles_fx`：abigail（Direct；line 1292）
- `abigail_shadow_buff_fx`：abigail（Direct；line 1292）
- `abigail_vex_debuff`：abigail（Direct；line 1292）
- `abigail_vex_shadow_debuff`：abigail（Direct；line 1292）
- `abigaillevelupfx`：abigail（Direct；line 1292）
- `prefabs/abigail_attack_fx.lua`：abigail（Direct；line 1292）
- `prefabs/abigailforcefield.lua`：abigail（Direct；line 1292）

### 行为
- `brains/abigailbrain.lua`：DoAction, Follow, Wander（prefabs/abigail.lua#abigail）


## 函数

### AbigailHealthDelta  [313–322]
- 归属：abigail

### AbleToAcceptTest  [328–330]
- 归属：abigail

### AddBonusHealth  [633–641]
- 归属：abigail

### AggressiveRetarget  [160–178]
- 归属：abigail

### ApplyDebuff  [380–403]
- 归属：abigail

### BecomeAggressive  [360–365]
- 归属：abigail

### BecomeDefensive  [367–374]
- 归属：abigail

### ChangeToGestalt  [725–735]
- 归属：abigail

### CommonRetarget  [127–134]
- 归属：abigail

### CreateDebuff  [1034–1184]
- 归属：（未归属）

### CustomCombatDamage  [307–311]
- 归属：abigail

### DefensiveRetarget  [136–158]
- 归属：abigail

### DelayedGhostScare  [494–499]
- 归属：abigail

### DoAppear  [324–326]
- 归属：abigail

### DoGhostAttackAt  [527–559]
- 归属：abigail

### DoGhostEscape  [466–484]
- 归属：abigail

### DoGhostHauntAt  [562–574]
- 归属：abigail

### DoGhostScare  [504–523]
- 归属：abigail

### DoRetaliationDamage  [990–997]
- 归属：abigail_retaliation

### DoShadowBurstBuff  [590–604]
- 归属：abigail

### IsWithinDefensiveRange  [99–105]
- 归属：abigail

### OnAttacked  [187–217]
- 归属：abigail

### OnBlocked  [219–225]
- 归属：abigail

### OnDeath  [227–231]
- 归属：abigail

### OnDebuffAdded  [332–340]
- 归属：abigail

### OnDebuffRemoved  [342–350]
- 归属：abigail

### OnDroppedTarget  [576–580]
- 归属：abigail

### OnExitLimbo  [459–463]
- 归属：abigail

### OnFadeToggleDirty  [736–743]
- 归属：abigail

### OnHealthChanged  [643–658]
- 归属：abigail

### OnLoad  [712–723]
- 归属：abigail

### OnRemoved  [233–235]
- 归属：abigail

### OnSave  [702–705]
- 归属：abigail

### SetMaxHealth  [59–74]
- 归属：abigail

### SetRetaliationTarget  [971–988]
- 归属：abigail_retaliation

### SetToGestalt  [660–680]
- 归属：abigail

### SetToNormal  [681–700]
- 归属：abigail

### SetTransparentPhysics  [107–117]
- 归属：abigail

### StartForceField  [180–185]
- 归属：abigail

### UndoTransparency  [45–57]
- 归属：abigail

### UpdateBonusHealth  [615–631]
- 归属：abigail

### UpdateDamage  [282–305]
- 归属：abigail, abigail_murder_buff

### UpdateGhostlyBondLevel  [76–95]
- 归属：abigail

### _auratest  [238–274]
- 归属：abigail

### abigail_murder_buff_fn  [1268–1290]
- 归属：abigail_murder_buff

### abigail_vex_debuff_fn  [1143–1181]
- 归属：（未归属）

### abigail_vex_hit_fn  [1189–1209]
- 归属：abigail_vex_hit

### addshadowvexplanardamge  [1063–1087]
- 归属：（未归属）

### apply_panic_fx  [486–492]
- 归属：abigail

### auratest  [277–279]
- 归属：abigail

### buff_OnAttached  [1089–1121]
- 归属：（未归属）

### buff_OnDetached  [1123–1141]
- 归属：（未归属）

### buff_OnExtended  [1051–1061]
- 归属：（未归属）

### calcabigailmaxhealthbonus  [606–613]
- 归属：abigail

### do_hit_fx  [1036–1043]
- 归属：（未归属）

### do_transparency  [41–43]
- 归属：abigail

### fn  [781–967]
- 归属：abigail

### getstatus  [583–588]
- 归属：abigail

### linktoplayer  [405–457]
- 归属：abigail

### murder_buff_OnAttached  [1220–1241]
- 归属：abigail_murder_buff

### murder_buff_OnDetached  [1243–1266]
- 归属：abigail_murder_buff

### murder_buff_OnExtended  [1213–1218]
- 归属：abigail_murder_buff

### on_ghostlybond_level_change  [352–358]
- 归属：abigail

### on_target_attacked  [1045–1049]
- 归属：（未归属）

### onload_bonushealth_task  [707–710]
- 归属：abigail

### onlostplayerlink  [376–378]
- 归属：abigail

### retaliationattack_fn  [999–1031]
- 归属：abigail_retaliation

### updatehealingbuffs  [745–779]
- 归属：abigail

