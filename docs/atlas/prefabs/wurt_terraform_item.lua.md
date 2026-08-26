# `prefabs/wurt_terraform_item.lua`

- 扫描角色：prefabs/wurt_terraform_item.lua
- 归属变体（6 个）：wurt_swampitem_lunar, wurt_swampitem_lunar_chargedfx, wurt_swampitem_shadow, wurt_swampitem_shadow_chargedfx, wurt_terraform_cast_debuff, wurt_terraform_projectile
## 关联

### 组件
- `components/complexprojectile.lua`：wurt_terraform_projectile（Direct；line 377）
- `components/debuff.lua`：wurt_terraform_cast_debuff（Direct；line 411）
- `components/equippable.lua`：wurt_swampitem_lunar, wurt_swampitem_shadow（Direct；line 206）
- `components/floater.lua`：wurt_swampitem_lunar, wurt_swampitem_shadow（HelperExpanded；line 196）
- `components/groundshadowhandler.lua`：wurt_terraform_projectile（Direct；line 366）
- `components/hauntable.lua`：wurt_swampitem_lunar, wurt_swampitem_shadow（HelperExpanded；line 233）
- `components/inspectable.lua`：wurt_swampitem_lunar, wurt_swampitem_shadow（Direct；line 211）
- `components/inventoryitem.lua`：wurt_swampitem_lunar, wurt_swampitem_shadow（Direct；line 214）
- `components/rechargeable.lua`：wurt_swampitem_lunar, wurt_swampitem_shadow（Direct；line 218）
- `components/reticule.lua`：wurt_swampitem_lunar, wurt_swampitem_shadow（Direct；line 181）
- `components/spellcaster.lua`：wurt_swampitem_lunar, wurt_swampitem_shadow（Direct；line 224）
- `components/timer.lua`：wurt_terraform_cast_debuff（Direct；line 414）
- `components/weapon.lua`：wurt_swampitem_lunar, wurt_swampitem_shadow（Direct；line 230）

### 预制体依赖
- `prefabs/wurt_swamp_terraformer.lua`：wurt_swampitem_lunar, wurt_swampitem_shadow（Direct；line 463,465）
- `wurt_merm_planar`：wurt_swampitem_lunar, wurt_swampitem_shadow（Direct；line 463,465）
- `wurt_swampitem_lunar_chargedfx`：wurt_swampitem_lunar, wurt_swampitem_shadow（Direct；line 463,465）
- `wurt_swampitem_shadow_chargedfx`：wurt_swampitem_lunar, wurt_swampitem_shadow（Direct；line 463,465）
- `wurt_terraform_cast_debuff`：wurt_swampitem_lunar, wurt_swampitem_shadow（Direct；line 463,465）
- `wurt_terraform_projectile`：wurt_swampitem_lunar, wurt_swampitem_shadow（Direct；line 463,465）


## 函数

### CLIENT_BombReticuleTargetFn  [30–42]
- 归属：wurt_swampitem_lunar, wurt_swampitem_shadow

### CanCastTerraformingSpell  [79–85]
- 归属：wurt_swampitem_lunar, wurt_swampitem_shadow

### CastPrevention_OnTimerDone  [389–393]
- 归属：wurt_terraform_cast_debuff

### CastTerraformingSpell  [46–77]
- 归属：wurt_swampitem_lunar, wurt_swampitem_shadow

### OnCharged  [110–117]
- 归属：wurt_swampitem_lunar, wurt_swampitem_shadow

### OnDischarged  [104–108]
- 归属：wurt_swampitem_lunar, wurt_swampitem_shadow

### OnEquip_CheckForChargedFX  [120–129]
- 归属：wurt_swampitem_lunar, wurt_swampitem_shadow

### OnHitTerraformer  [317–332]
- 归属：wurt_terraform_projectile

### OnHit_Lunar  [264–291]
- 归属：wurt_swampitem_lunar

### PlayProjectileSpawnSound  [334–336]
- 归属：wurt_terraform_projectile

### add_charged_fx  [88–96]
- 归属：wurt_swampitem_lunar, wurt_swampitem_shadow

### cant_terraform_debuff_fn  [394–419]
- 归属：wurt_terraform_cast_debuff

### charged_fx_common  [422–448]
- 归属：wurt_swampitem_lunar_chargedfx, wurt_swampitem_shadow_chargedfx

### item_is_wurt_terraform_equipment  [25–27]
- 归属：wurt_swampitem_lunar, wurt_swampitem_shadow

### on_item_putininventory  [148–164]
- 归属：wurt_swampitem_lunar, wurt_swampitem_shadow

### onequip  [130–138]
- 归属：wurt_swampitem_lunar, wurt_swampitem_shadow

### remove_charged_fx  [97–102]
- 归属：wurt_swampitem_lunar, wurt_swampitem_shadow

### terraform_projectile  [338–386]
- 归属：wurt_terraform_projectile

### unequip  [139–145]
- 归属：wurt_swampitem_lunar, wurt_swampitem_shadow

### wurt_swampbomb_lunar  [293–314]
- 归属：wurt_swampitem_lunar

### wurt_swampbomb_shadow  [239–259]
- 归属：wurt_swampitem_shadow

### wurt_swampitem_lunar_fx  [454–461]
- 归属：wurt_swampitem_lunar_chargedfx

### wurt_swampitem_shadow_fx  [450–452]
- 归属：wurt_swampitem_shadow_chargedfx

### wurt_terraformer_fn  [167–236]
- 归属：wurt_swampitem_lunar, wurt_swampitem_shadow

