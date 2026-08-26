# `prefabs/worm.lua`

- 扫描角色：prefabs/worm.lua
- 归属变体（2 个）：worm, yots_worm
## 关联

### 组件
- `components/acidinfusible.lua`：worm, yots_worm（Direct；line 360）
- `components/burnable.lua`：yots_worm（HelperExpanded；line 505）
- `components/combat.lua`：worm, yots_worm（Direct；line 320）
- `components/drownable.lua`：worm, yots_worm（Direct；line 336）
- `components/eater.lua`：worm, yots_worm（Direct；line 338）
- `components/health.lua`：worm, yots_worm（Direct；line 317）
- `components/inspectable.lua`：worm, yots_worm（Direct；line 354）
- `components/inventory.lua`：worm, yots_worm（Direct；line 352）
- `components/knownlocations.lua`：worm, yots_worm（Direct；line 350）
- `components/locomotor.lua`：worm, yots_worm（Direct；line 330）
- `components/lootdropper.lua`：worm, yots_worm（Direct；line 357）
- `components/pickable.lua`：worm, yots_worm（Direct；line 341）
- `components/playerprox.lua`：worm, yots_worm（Direct；line 345）
- `components/sanityaura.lua`：worm, yots_worm（Direct；line 327）

### 状态图
- `stategraphs/SGworm.lua`：worm, yots_worm（Direct；line 374）

### 大脑
- `brains/wormbrain.lua`：worm, yots_worm（Direct；line 375）

### 预制体依赖
- `monstermeat`：worm, yots_worm（Direct；line 511,512）
- `prefabs/wormlight.lua`：worm, yots_worm（Direct；line 511,512）
- `worm_ruinsrespawner_inst`：worm（Direct；line 511）
- `yots_redlantern`：yots_worm（Direct；line 512）

### 行为
- `brains/wormbrain.lua`：ChaseAndAttack, Leash, StandStill, Wander（prefabs/worm.lua#worm, prefabs/worm.lua#yots_worm）


## 函数

### CustomOnHaunt  [239–258]
- 归属：worm, yots_worm

### IsAlive  [111–113]
- 归属：worm, yots_worm

### IsWorm  [228–230]
- 归属：worm, yots_worm

### LookForHome  [190–213]
- 归属：worm, yots_worm

### OnLightDirty  [92–97]
- 归属：worm, yots_worm

### OnUpdateLight  [68–90]
- 归属：worm, yots_worm

### areaislush  [180–182]
- 归属：worm, yots_worm

### default_fn  [389–399]
- 归属：worm

### displaynamefn  [162–169]
- 归属：worm, yots_worm

### fncommon  [266–383]
- 归属：worm, yots_worm

### getstatus  [171–175]
- 归属：worm, yots_worm

### lootsetfn  [260–264]
- 归属：worm, yots_worm

### notclaimed  [185–188]
- 归属：worm, yots_worm

### onattacked  [232–237]
- 归属：worm, yots_worm

### onpickedfn  [147–160]
- 归属：worm, yots_worm

### onruinsrespawn  [385–387]
- 归属：（未归属）

### playerfar  [221–226]
- 归属：worm, yots_worm

### playernear  [215–219]
- 归属：worm, yots_worm

### retargetfn  [118–130]
- 归属：worm, yots_worm

### shouldKeepTarget  [132–145]
- 归属：worm, yots_worm

### turnofflight  [105–109]
- 归属：worm, yots_worm

### turnonlight  [99–103]
- 归属：worm, yots_worm

### yots_fn  [487–508]
- 归属：yots_worm

### yots_onnewstate  [481–485]
- 归属：yots_worm

### yots_retargetfn  [402–431]
- 归属：yots_worm

### yots_shouldKeepTarget  [433–479]
- 归属：yots_worm

