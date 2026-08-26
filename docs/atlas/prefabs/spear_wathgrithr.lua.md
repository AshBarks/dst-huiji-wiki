# `prefabs/spear_wathgrithr.lua`

- 扫描角色：prefabs/spear_wathgrithr.lua
- 归属变体（5 个）：spear_wathgrithr, spear_wathgrithr_lightning, spear_wathgrithr_lightning_charged, spear_wathgrithr_lightning_fx, spear_wathgrithr_lightning_lunge_fx
## 关联

### 组件
- `components/aoespell.lua`：spear_wathgrithr_lightning, spear_wathgrithr_lightning_charged（Direct；line 492）
- `components/aoetargeting.lua`：spear_wathgrithr_lightning, spear_wathgrithr_lightning_charged（Direct；line 428）
- `components/aoeweapon_lunge.lua`：spear_wathgrithr_lightning, spear_wathgrithr_lightning_charged（Direct；line 481）
- `components/colouradder.lua`：spear_wathgrithr_lightning_fx（Direct；line 681）
- `components/equippable.lua`：spear_wathgrithr, spear_wathgrithr_lightning, spear_wathgrithr_lightning_charged（Direct；line 396）
- `components/finiteuses.lua`：spear_wathgrithr, spear_wathgrithr_lightning, spear_wathgrithr_lightning_charged（Direct；line 391）
- `components/floater.lua`：spear_wathgrithr, spear_wathgrithr_lightning, spear_wathgrithr_lightning_charged（HelperExpanded；line 363）
- `components/hauntable.lua`：spear_wathgrithr, spear_wathgrithr_lightning, spear_wathgrithr_lightning_charged（HelperExpanded；line 400）
- `components/highlightchild.lua`：spear_wathgrithr_lightning_fx（Direct；line 673）
- `components/inspectable.lua`：spear_wathgrithr, spear_wathgrithr_lightning, spear_wathgrithr_lightning_charged（Direct；line 380）
- `components/inventoryitem.lua`：spear_wathgrithr, spear_wathgrithr_lightning, spear_wathgrithr_lightning_charged（Direct；line 381）
- `components/planardamage.lua`：spear_wathgrithr, spear_wathgrithr_lightning, spear_wathgrithr_lightning_charged（Direct；line 387）
- `components/rechargeable.lua`：spear_wathgrithr_lightning, spear_wathgrithr_lightning_charged（Direct；line 495）
- `components/upgradeable.lua`：spear_wathgrithr_lightning（Direct；line 503）
- `components/weapon.lua`：spear_wathgrithr, spear_wathgrithr_lightning, spear_wathgrithr_lightning_charged（Direct；line 383）

### 预制体依赖
- `prefabs/reticuleline.lua`：spear_wathgrithr_lightning, spear_wathgrithr_lightning_charged（Direct；line 693,694）
- `reticulelineping`：spear_wathgrithr_lightning, spear_wathgrithr_lightning_charged（Direct；line 693,694）
- `spear_wathgrithr_lightning_charged`：spear_wathgrithr_lightning（Direct；line 693）
- `spear_wathgrithr_lightning_fx`：spear_wathgrithr_lightning_charged（Direct；line 694）
- `spear_wathgrithr_lightning_lunge_fx`：spear_wathgrithr_lightning, spear_wathgrithr_lightning_charged（Direct；line 693,694）

### 生成引用
- `spear_wathgrithr_lightning_charged`：spear_wathgrithr_lightning（Direct；line 253）
- `spear_wathgrithr_lightning_fx`：spear_wathgrithr_lightning_charged（Direct；line 524）
- `spear_wathgrithr_lightning_lunge_fx`：spear_wathgrithr_lightning, spear_wathgrithr_lightning_charged（Direct；line 175）


## 函数

### BasicSpearFn  [409–419]
- 归属：spear_wathgrithr

### CommonFn  [343–407]
- 归属：spear_wathgrithr, spear_wathgrithr_lightning, spear_wathgrithr_lightning_charged

### FX_OnAnimOver  [591–598]
- 归属：spear_wathgrithr_lightning_lunge_fx

### FX_OnUpdate  [578–589]
- 归属：spear_wathgrithr_lightning_lunge_fx

### FxFn  [649–686]
- 归属：spear_wathgrithr_lightning_fx

### LightningCharged_OnEntitySleep  [337–339]
- 归属：spear_wathgrithr_lightning_charged

### LightningCharged_OnEntityWake  [327–335]
- 归属：spear_wathgrithr_lightning_charged

### LightningCharged_OnStopFloating  [322–325]
- 归属：spear_wathgrithr_lightning_charged

### LightningCharged_SetFxOwner  [282–316]
- 归属：spear_wathgrithr_lightning_charged

### LightningSpearChargedFn  [562–574]
- 归属：spear_wathgrithr_lightning_charged

### LightningSpearCommonFn_Base  [421–439]
- 归属：spear_wathgrithr_lightning, spear_wathgrithr_lightning_charged

### LightningSpearCommonFn_Charged  [445–462]
- 归属：spear_wathgrithr_lightning_charged

### LightningSpearCommonFn_Normal  [441–443]
- 归属：spear_wathgrithr_lightning

### LightningSpearFn  [548–560]
- 归属：spear_wathgrithr_lightning

### LightningSpearPostInitFn_Base  [464–498]
- 归属：spear_wathgrithr_lightning, spear_wathgrithr_lightning_charged

### LightningSpearPostInitFn_Charged  [511–546]
- 归属：spear_wathgrithr_lightning_charged

### LightningSpearPostInitFn_Normal  [500–507]
- 归属：spear_wathgrithr_lightning

### Lightning_CanBeUpgraded  [244–246]
- 归属：spear_wathgrithr_lightning

### Lightning_HasElectric  [83–85]
- 归属：spear_wathgrithr_lightning, spear_wathgrithr_lightning_charged

### Lightning_OnAttack  [149–153]
- 归属：spear_wathgrithr_lightning, spear_wathgrithr_lightning_charged

### Lightning_OnCharged  [201–207]
- 归属：spear_wathgrithr_lightning, spear_wathgrithr_lightning_charged

### Lightning_OnDischarged  [197–199]
- 归属：spear_wathgrithr_lightning, spear_wathgrithr_lightning_charged

### Lightning_OnLunged  [174–182]
- 归属：spear_wathgrithr_lightning, spear_wathgrithr_lightning_charged

### Lightning_OnLungedHit  [184–195]
- 归属：spear_wathgrithr_lightning, spear_wathgrithr_lightning_charged

### Lightning_OnPreLunge  [165–172]
- 归属：spear_wathgrithr_lightning, spear_wathgrithr_lightning_charged

### Lightning_OnUpgraded  [248–270]
- 归属：spear_wathgrithr_lightning

### Lightning_OverrideStimuliFn  [145–147]
- 归属：spear_wathgrithr_lightning, spear_wathgrithr_lightning_charged

### Lightning_ResetElectric  [161–163]
- 归属：spear_wathgrithr_lightning, spear_wathgrithr_lightning_charged

### Lightning_ReticuleMouseTargetFn  [216–228]
- 归属：spear_wathgrithr_lightning, spear_wathgrithr_lightning_charged

### Lightning_ReticuleTargetFn  [211–214]
- 归属：spear_wathgrithr_lightning, spear_wathgrithr_lightning_charged

### Lightning_ReticuleUpdatePositionFn  [230–240]
- 归属：spear_wathgrithr_lightning, spear_wathgrithr_lightning_charged

### Lightning_SpellFn  [157–159]
- 归属：spear_wathgrithr_lightning, spear_wathgrithr_lightning_charged

### LungueTrailFxFn  [600–645]
- 归属：spear_wathgrithr_lightning_lunge_fx

### OnEquip  [87–119]
- 归属：spear_wathgrithr, spear_wathgrithr_lightning, spear_wathgrithr_lightning_charged

### OnUnequip  [121–141]
- 归属：spear_wathgrithr, spear_wathgrithr_lightning, spear_wathgrithr_lightning_charged

### PushIdleLoop  [318–320]
- 归属：spear_wathgrithr_lightning_charged

### RefreshAttunedSkills  [46–65]
- 归属：spear_wathgrithr, spear_wathgrithr_lightning, spear_wathgrithr_lightning_charged

### WatchSkillRefresh  [67–77]
- 归属：spear_wathgrithr, spear_wathgrithr_lightning, spear_wathgrithr_lightning_charged

