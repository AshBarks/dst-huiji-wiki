# `prefabs/carrat.lua`

- 扫描角色：prefabs/carrat.lua
- 归属变体（2 个）：carrat, carrat_planted
## 关联

### 组件
- `components/burnable.lua`：carrat, carrat_planted（Direct/HelperExpanded；line 785,914）
- `components/combat.lua`：carrat（Direct；line 485,780）
- `components/cookable.lua`：carrat（Direct；line 481,767）
- `components/drownable.lua`：carrat（Direct；line 476,757）
- `components/eater.lua`：carrat（Direct/HelperExpanded；line 763,805）
- `components/entitytracker.lua`：carrat（Direct；line 741）
- `components/freezable.lua`：carrat（HelperExpanded；line 491,795）
- `components/hauntable.lua`：carrat, carrat_planted（Direct/HelperExpanded；line 467,802,920）
- `components/health.lua`：carrat（Direct；line 773）
- `components/homeseeker.lua`：carrat（Direct；line 771）
- `components/inspectable.lua`：carrat, carrat_planted（Direct；line 797,905）
- `components/inventoryitem.lua`：carrat（Direct；line 718）
- `components/locomotor.lua`：carrat（Direct；line 472,753）
- `components/lootdropper.lua`：carrat（Direct；line 478,777）
- `components/named.lua`：carrat（Direct；line 736）
- `components/pickable.lua`：carrat, carrat_planted（Direct；line 249,908）
- `components/propagator.lua`：carrat, carrat_planted（HelperExpanded；line 792,918）
- `components/sleeper.lua`：carrat（Direct；line 488,798）
- `components/tradable.lua`：carrat（Direct；line 489,800）
- `components/workable.lua`：carrat, carrat_planted（Direct；line 254,923）
- `components/yotc_racecompetitor.lua`：carrat（Direct；line 405）
- `components/yotc_racestats.lua`：carrat（Direct；line 738）

### 状态图
- `stategraphs/SGcarrat.lua`：carrat（Direct；line 759）

### 大脑
- `brains/carratbrain.lua`：carrat（Direct；line 502,761）

### 预制体依赖
- `carrat_planted`：carrat（Direct；line 940）
- `carrot_seeds`：carrat（Direct；line 940）
- `plantmeat`：carrat（Direct；line 940）
- `plantmeat_cooked`：carrat（Direct；line 940）
- `prefabs/carrat.lua`：carrat_planted（Direct；line 941）
- `redpouch_yotc`：carrat（Direct；line 940）

### 行为
- `brains/carratbrain.lua`：DoAction, FaceEntity, Leash, Panic, RunAway, Wander（prefabs/carrat.lua#carrat）


## 函数

### GetColorFromFood  [646–649]
- 归属：carrat

### OnMusicStateDirty  [148–156]
- 归属：carrat

### _dodirectiongym  [543–547]
- 归属：carrat

### _doreactiongym  [549–553]
- 归属：carrat

### _dospeedgym  [537–541]
- 归属：carrat

### _dostaminagym  [555–559]
- 归属：carrat

### client_get_drop_action_string  [508–523]
- 归属：carrat

### common_onload  [124–146]
- 归属：carrat, carrat_planted

### common_onsave  [113–122]
- 归属：carrat, carrat_planted

### common_setcolor  [84–111]
- 归属：carrat, carrat_planted

### docarratfailtalk  [158–174]
- 归属：carrat

### find_yotc_race_startentity  [315–326]
- 归属：carrat

### fn  [663–818]
- 归属：carrat

### full_race_over  [375–380]
- 归属：carrat

### getcarratfromtrap  [602–618]
- 归属：carrat

### go_to_emerged  [445–506]
- 归属：carrat

### go_to_submerged  [204–273]
- 归属：carrat

### on_cooked_fn  [275–277]
- 归属：carrat

### on_dropped  [382–443]
- 归属：carrat

### on_planted_prefab_dug_up  [871–881]
- 归属：carrat_planted

### on_planted_prefab_ignite  [857–869]
- 归属：carrat_planted

### on_planted_prefab_picked  [848–855]
- 归属：carrat_planted

### on_submerged_dug_up  [190–193]
- 归属：carrat

### on_submerged_haunt_fn  [195–197]
- 归属：carrat

### on_submerged_ignite  [180–183]
- 归属：carrat

### on_submerged_picked  [185–188]
- 归属：carrat

### planted_fn  [883–938]
- 归属：carrat_planted

### play_special_submerged_idle  [199–202]
- 归属：carrat, carrat_planted

### race_begun  [328–365]
- 归属：carrat

### reached_finish_line  [367–373]
- 归属：carrat

### setbeefalocarratrat  [658–660]
- 归属：carrat

### settrapdata  [592–600]
- 归属：carrat

### spawn_carrat_from_planted  [824–846]
- 归属：carrat_planted

### spread_stats  [525–534]
- 归属：carrat

### yotc_drop_prize_on_death  [561–572]
- 归属：carrat

### yotc_nighttime_degrade_test  [574–590]
- 归属：carrat

### yotc_on_inventory  [279–311]
- 归属：carrat

### yotc_oneatfn  [651–656]
- 归属：carrat

