# `prefabs/wobybig.lua`

- 扫描角色：prefabs/wobybig.lua
- 归属变体（1 个）：wobybig
## 关联

### 组件
- `components/burnable.lua`：wobybig（HelperExpanded；line 1002）
- `components/colouradder.lua`：wobybig（Direct；line 1017）
- `components/colourtweener.lua`：wobybig（Direct；line 1016）
- `components/container.lua`：wobybig（Direct；line 1010）
- `components/drownable.lua`：wobybig（Direct；line 1014）
- `components/eater.lua`：wobybig（Direct；line 973）
- `components/embarker.lua`：wobybig（Direct；line 1013）
- `components/follower.lua`：wobybig（Direct；line 984）
- `components/freezable.lua`：wobybig（HelperExpanded；line 1003）
- `components/hauntable.lua`：wobybig（HelperExpanded；line 1019）
- `components/hunger.lua`：wobybig（Direct；line 997）
- `components/inspectable.lua`：wobybig（Direct；line 981）
- `components/locomotor.lua`：wobybig（Direct；line 1005）
- `components/maprevealable.lua`：wobybig（Direct；line 978）
- `components/rideable.lua`：wobybig（Direct；line 988）
- `components/sleeper.lua`：wobybig（Direct；line 992）
- `components/spawnfader.lua`：wobybig（Direct；line 951）
- `components/timer.lua`：wobybig（Direct；line 982）
- `components/wobyrack.lua`：wobybig（Direct；line 230）

### 状态图
- `stategraphs/SGwobybig.lua`：wobybig（Direct；line 1022）

### 大脑
- `brains/wobybigbrain.lua`：wobybig（Direct；line 1021）

### 预制体依赖
- `globalmapiconunderfog`：wobybig（Direct；line 1087）
- `prefabs/pet_hunger_classified.lua`：wobybig（Direct；line 1087）
- `prefabs/woby_commands_classified.lua`：wobybig（Direct；line 1087）
- `prefabs/wobysmall.lua`：wobybig（Direct；line 1087）
- `woby_dash_shadow_fx`：wobybig（Direct；line 1087）
- `woby_dash_silhouette_fx`：wobybig（Direct；line 1087）
- `woby_rack_container`：wobybig（Direct；line 1087）
- `woby_rack_swap_fx`：wobybig（Direct；line 1087）

### 生成引用
- `prefabs/pet_hunger_classified.lua`：wobybig（Direct；line 699）
- `prefabs/woby_commands_classified.lua`：wobybig（Direct；line 716）
- `woby_rack_swap_fx`：wobybig（Direct；line 236,237）

### 行为
- `brains/wobybigbrain.lua`：FaceEntity, Follow, Wander（prefabs/wobybig.lua#wobybig）


## 函数

### ApplyBuildOverrides  [108–110]
- 归属：wobybig

### ApplySmallBuildOverrides  [113–130]
- 归属：wobybig

### CheckLunarPower  [374–378]
- 归属：wobybig

### ClearBuildOverrides  [101–105]
- 归属：wobybig

### ClearForagerQueue  [611–622]
- 归属：wobybig

### ClearSprintHungerBurn  [356–361]
- 归属：wobybig

### CustomFoodStatsMod  [349–354]
- 归属：wobybig

### DoRiderSleep  [437–439]
- 归属：（未归属）

### EnableRack  [227–274]
- 归属：wobybig

### FinishTransformation  [775–878]
- 归属：wobybig

### GetForagerTarget  [584–594]
- 归属：wobybig

### HasEndurance  [313–318]
- 归属：wobybig

### HideRackItem  [218–225]
- 归属：wobybig

### IsAllowedToQueueForaging  [507–521]
- 归属：wobybig

### IsLeaderSleeping  [883–886]
- 归属：wobybig

### IsLeaderTellingStory  [888–891]
- 归属：wobybig

### IsLunarPowered  [306–311]
- 归属：wobybig

### LinkToPlayer  [694–739]
- 归属：wobybig

### OnAnyClose  [184–188]
- 归属：wobybig

### OnAnyOpen  [178–182]
- 归属：wobybig

### OnDash  [485–497]
- 归属：wobybig

### OnEat  [901–905]
- 归属：wobybig

### OnHungerDelta  [343–347]
- 归属：wobybig

### OnPlayerLinkDespawn  [741–773]
- 归属：wobybig

### OnPlayerNewState  [523–548]
- 归属：wobybig

### OnPreLoad  [276–280]
- 归属：wobybig

### OnRiderChanged  [441–475]
- 归属：wobybig

### OnRiderSleep  [477–483]
- 归属：wobybig

### OnStarving  [433–435]
- 归属：wobybig

### OnSuccessfulPraisableAction  [626–630]
- 归属：wobybig

### OnWobySkinChanged  [133–146]
- 归属：wobybig

### QueueForagerTarget  [553–567]
- 归属：wobybig

### RefreshAttunedSkills  [634–692]
- 归属：wobybig

### RemoveCurrentForagerTarget  [580–582]
- 归属：wobybig

### RemoveForagerTarget  [569–578]
- 归属：wobybig

### RestoreCharacterCollisions  [907–909]
- 归属：wobybig

### SetAlignmentBuild  [148–173]
- 归属：wobybig

### SetBaseRunSpeed  [329–334]
- 归属：wobybig

### SetBaseRunSpeedFromHunger  [336–341]
- 归属：wobybig

### SetRackFxOwner  [190–207]
- 归属：wobybig

### SetRunSpeed  [320–327]
- 归属：wobybig

### SetSprinting  [410–431]
- 归属：wobybig

### ShouldSleep  [897–899]
- 归属：wobybig

### ShouldWakeUp  [893–895]
- 归属：wobybig

### ShowRackItem  [209–216]
- 归属：wobybig

### StartWatchingLunarPower  [380–392]
- 归属：wobybig

### StopWatchingLunarPower  [394–408]
- 归属：wobybig

### TimeoutForageTarget  [503–505]
- 归属：wobybig

### TriggerTransformation  [284–304]
- 归属：wobybig

### UpdateOwnerNewStateListener  [596–609]
- 归属：wobybig

### UpdateSprintHungerBurn  [363–372]
- 归属：wobybig

### _ApplyAlignmentOverrides_Internal  [60–81]
- 归属：wobybig

### _ApplySmallBuildOverrides_Internal  [84–98]
- 归属：wobybig

### fn  [911–1085]
- 归属：wobybig

