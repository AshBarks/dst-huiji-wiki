# `prefabs/messagebottle.lua`

- 扫描角色：prefabs/messagebottle.lua
- 归属变体（4 个）：gelblob_bottle, messagebottle, messagebottle_throwable, messagebottleempty
## 关联

### 组件
- `components/bottler.lua`：messagebottleempty（Direct；line 258）
- `components/complexprojectile.lua`：gelblob_bottle, messagebottle_throwable（Direct；line 330,470）
- `components/equippable.lua`：gelblob_bottle, messagebottle_throwable（Direct；line 324,465）
- `components/floater.lua`：gelblob_bottle, messagebottle, messagebottle_throwable, messagebottleempty（HelperExpanded；line 151,247,448）
- `components/inspectable.lua`：gelblob_bottle, messagebottle, messagebottle_throwable, messagebottleempty（Direct；line 171,255,456）
- `components/inventoryitem.lua`：gelblob_bottle, messagebottle, messagebottle_throwable, messagebottleempty（Direct；line 172,256,457）
- `components/mapspotrevealer.lua`：messagebottle, messagebottle_throwable（Direct；line 177）
- `components/stackable.lua`：gelblob_bottle, messagebottleempty（Direct；line 261,459）
- `components/waterproofer.lua`：gelblob_bottle, messagebottle, messagebottle_throwable, messagebottleempty（Direct；line 174,264,462）

### 预制体依赖
- `gelblob_bottle`：messagebottleempty（Direct；line 485）
- `gelblob_small_fx`：gelblob_bottle（Direct；line 487）
- `messagebottle_throwable`：messagebottle（Direct；line 484）
- `messagebottleempty`：messagebottle（Direct；line 484）


## 函数

### GelBlobBottle_OnEquip  [374–378]
- 归属：gelblob_bottle

### GelBlobBottle_OnHit  [385–401]
- 归属：gelblob_bottle

### GelBlobBottle_OnStartFloating  [421–423]
- 归属：gelblob_bottle

### GelBlobBottle_OnStopFloating  [425–427]
- 归属：gelblob_bottle

### GelBlobBottle_OnThrown  [403–419]
- 归属：gelblob_bottle

### GelBlobBottle_OnUnequip  [380–383]
- 归属：gelblob_bottle

### OnBottle  [212–228]
- 归属：messagebottleempty

### OnHit  [285–297]
- 归属：messagebottle_throwable

### ShouldForceMapReveal  [86–111]
- 归属：messagebottle, messagebottle_throwable

### bobbottlefn  [344–370]
- 归属：（未归属）

### commonmakebottle  [139–191]
- 归属：messagebottle, messagebottle_throwable

### emptybottlefn  [232–272]
- 归属：messagebottleempty

### gelblobbottlefn  [429–481]
- 归属：gelblob_bottle

### getrevealtargetpos  [48–55]
- 归属：messagebottle, messagebottle_throwable

### messagebottlefn  [193–195]
- 归属：messagebottle

### ondropped  [44–46]
- 归属：messagebottle, messagebottle_throwable, messagebottleempty

### ondropped_empty  [206–208]
- 归属：（未归属）

### onequip  [274–278]
- 归属：messagebottle_throwable

### onplayerfinishedreadingnote  [70–84]
- 归属：messagebottle, messagebottle_throwable

### onthrown  [299–315]
- 归属：messagebottle_throwable

### onunequip  [280–283]
- 归属：messagebottle_throwable

### playidleanim  [35–42]
- 归属：messagebottle, messagebottle_throwable

### playidleanim_empty  [197–204]
- 归属：messagebottleempty

### prereveal  [113–137]
- 归属：messagebottle, messagebottle_throwable

### throwing_common_postinit  [317–321]
- 归属：messagebottle_throwable

### throwing_master_postinit  [323–338]
- 归属：messagebottle_throwable

### throwingbottlefn  [340–342]
- 归属：messagebottle_throwable

### turn_empty  [57–68]
- 归属：messagebottle, messagebottle_throwable

