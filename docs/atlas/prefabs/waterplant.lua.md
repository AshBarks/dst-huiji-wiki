# `prefabs/waterplant.lua`

- 扫描角色：prefabs/waterplant.lua
- 归属变体（3 个）：waterplant, waterplant_base, waterplant_spawner_rough
## 关联

### 组件
- `components/burnable.lua`：waterplant（HelperExpanded；line 479）
- `components/childspawner.lua`：waterplant（Direct；line 468）
- `components/colouradder.lua`：waterplant（Direct；line 437）
- `components/combat.lua`：waterplant（Direct；line 450）
- `components/equippable.lua`：waterplant（Direct；line 214）
- `components/floater.lua`：waterplant（HelperExpanded；line 408）
- `components/freezable.lua`：waterplant, waterplant_base（HelperExpanded；line 485,565）
- `components/harvestable.lua`：waterplant（Direct；line 440）
- `components/health.lua`：waterplant（Direct；line 457）
- `components/inspectable.lua`：waterplant（Direct；line 466）
- `components/inventory.lua`：waterplant（Direct；line 464）
- `components/inventoryitem.lua`：waterplant（Direct；line 211）
- `components/lootdropper.lua`：waterplant（Direct；line 461）
- `components/propagator.lua`：waterplant（HelperExpanded；line 484）
- `components/shaveable.lua`：waterplant（Direct；line 445）
- `components/sleeper.lua`：waterplant（Direct；line 435）
- `components/timer.lua`：waterplant（Direct；line 477）
- `components/waterphysics.lua`：waterplant（HelperExpanded；line 374）
- `components/weapon.lua`：waterplant（Direct；line 206）

### 状态图
- `stategraphs/SGwaterplant.lua`：waterplant（Direct；line 509）

### 大脑
- `brains/waterplantbrain.lua`：waterplant（Direct；line 511）

### 预制体依赖
- `barnacle`：waterplant, waterplant_spawner_rough（Direct；line 583,585）
- `barnacle_cooked`：waterplant, waterplant_spawner_rough（Direct；line 583,585）
- `prefabs/waterplant_planter.lua`：waterplant, waterplant_spawner_rough（Direct；line 583,585）
- `prefabs/waterplant_rock.lua`：waterplant, waterplant_spawner_rough（Direct；line 583,585）
- `waterplant_base`：waterplant, waterplant_spawner_rough（Direct；line 583,585）
- `waterplant_bomb`：waterplant, waterplant_spawner_rough（Direct；line 583,585）
- `waterplant_pollen_fx`：waterplant, waterplant_spawner_rough（Direct；line 583,585）
- `waterplant_projectile`：waterplant, waterplant_spawner_rough（Direct；line 583,585）

### 生成引用
- `prefabs/waterplant_rock.lua`：waterplant（Direct；line 129,257）
- `waterplant_base`：waterplant（Direct；line 421）
- `waterplant_pollen_fx`：waterplant（Direct；line 304）

### 行为
- `brains/waterplantbrain.lua`：FaceEntity, StandAndAttack（prefabs/waterplant.lua#waterplant）


## 函数

### basefn  [539–570]
- 归属：waterplant_base

### can_shave  [165–171]
- 归属：waterplant

### client_on_base_replicated  [532–537]
- 归属：waterplant_base

### equip_ranged_weapon  [198–218]
- 归属：waterplant

### find_and_attack_nearby_player  [229–238]
- 归属：waterplant

### fn  [364–530]
- 归属：waterplant

### go_to_sleep  [321–323]
- 归属：waterplant

### go_to_stage  [112–126]
- 归属：waterplant

### keeptarget  [192–196]
- 归属：waterplant

### on_attacked  [224–226]
- 归属：waterplant

### on_burnt  [248–259]
- 归属：waterplant
- 掉落锚点：252:barnacle_cooked

### on_collide  [280–290]
- 归属：waterplant

### on_dropped_target  [296–301]
- 归属：waterplant

### on_entity_sleep  [337–342]
- 归属：waterplant

### on_entity_wake  [344–348]
- 归属：waterplant

### on_extinguish  [261–263]
- 归属：waterplant

### on_frozen  [265–267]
- 归属：waterplant

### on_grown  [151–163]
- 归属：waterplant

### on_harvested  [136–149]
- 归属：waterplant

### on_ignited  [240–246]
- 归属：waterplant

### on_landed_initialize  [359–361]
- 归属：waterplant

### on_load  [354–357]
- 归属：waterplant

### on_new_combat_target  [292–294]
- 归属：waterplant

### on_own_fish  [220–222]
- 归属：waterplant

### on_save  [350–352]
- 归属：waterplant

### on_shaved  [173–186]
- 归属：waterplant

### on_timer_finished  [329–335]
- 归属：waterplant

### on_unfrozen  [269–278]
- 归属：waterplant

### on_wakeup  [325–327]
- 归属：waterplant

### release_all_fish  [315–317]
- 归属：waterplant

### retarget  [188–190]
- 归属：waterplant

### revert_to_rock  [128–134]
- 归属：waterplant

### set_flower_type  [47–66]
- 归属：waterplant

### set_target  [80–90]
- 归属：waterplant

### spawn_pollen_cloud  [303–313]
- 归属：waterplant

### spawnerfn  [572–581]
- 归属：waterplant_spawner_rough

### syncanim  [68–71]
- 归属：waterplant

### syncanimpush  [73–76]
- 归属：waterplant

### update_barnacle_layers  [92–110]
- 归属：waterplant

