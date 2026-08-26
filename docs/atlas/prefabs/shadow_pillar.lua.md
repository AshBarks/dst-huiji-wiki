# `prefabs/shadow_pillar.lua`

- 扫描角色：prefabs/shadow_pillar.lua
- 归属变体（4 个）：shadow_pillar, shadow_pillar_base_fx, shadow_pillar_spell, shadow_pillar_target
## 关联

### 组件
- `components/entitytracker.lua`：shadow_pillar, shadow_pillar_target（Direct；line 309,575）
- `components/rooted.lua`：shadow_pillar_target（Direct；line 482）
- `components/timer.lua`：shadow_pillar, shadow_pillar_target（Direct；line 311,577）

### 预制体依赖
- `ocean_splash_med1`：shadow_pillar（Direct；line 774）
- `ocean_splash_med2`：shadow_pillar（Direct；line 774）
- `prefabs/shadow_glob_fx.lua`：shadow_pillar_spell（Direct；line 777）
- `prefabs/shadow_pillar.lua`：shadow_pillar_spell（Direct；line 777）
- `sanity_lower`：shadow_pillar（Direct；line 774）
- `sanity_raise`：shadow_pillar（Direct；line 774）
- `shadow_pillar_base_fx`：shadow_pillar（Direct；line 774）
- `shadow_pillar_target`：shadow_pillar_spell（Direct；line 777）

### 生成引用
- `prefabs/shadow_glob_fx.lua`：shadow_pillar_spell（Direct；line 604）
- `prefabs/shadow_pillar.lua`：shadow_pillar_spell（Direct；line 702）
- `sanity_lower`：shadow_pillar（Direct；line 158）
- `sanity_raise`：shadow_pillar（Direct；line 135）
- `shadow_pillar_base_fx`：shadow_pillar（Direct；line 123,254）
- `shadow_pillar_target`：shadow_pillar_spell（Direct；line 689）


## 函数

### Base_KillFX  [370–373]
- 归属：shadow_pillar_base_fx

### CalcTargetDuration  [33–38]
- 归属：shadow_pillar, shadow_pillar_target

### CreateRipples  [328–346]
- 归属：shadow_pillar_base_fx

### DoLower  [84–94]
- 归属：shadow_pillar

### DoPillars  [721–739]
- 归属：shadow_pillar_spell

### DoPillarsTarget  [668–714]
- 归属：shadow_pillar_spell

### DoRaise  [71–82]
- 归属：shadow_pillar

### DoRipple  [348–352]
- 归属：shadow_pillar_base_fx

### DoSplash  [65–69]
- 归属：shadow_pillar

### GetNextFlipped  [58–63]
- 归属：shadow_pillar

### GetNextVariation  [48–55]
- 归属：shadow_pillar

### IsNearOther  [659–666]
- 归属：shadow_pillar_spell

### Pillar_OnDispell  [96–104]
- 归属：shadow_pillar

### Pillar_OnLoad  [239–271]
- 归属：shadow_pillar

### Pillar_OnLoadPostPass  [273–281]
- 归属：shadow_pillar

### Pillar_OnSave  [233–237]
- 归属：shadow_pillar

### Pillar_OnSetTarget  [172–198]
- 归属：shadow_pillar

### Pillar_OnTargetRemoved  [106–118]
- 归属：shadow_pillar

### Pillar_OnTimerDone  [130–163]
- 归属：shadow_pillar

### Pillar_SetDelay  [165–170]
- 归属：shadow_pillar

### Pillar_SetTarget  [200–231]
- 归属：shadow_pillar

### PreRaise  [120–127]
- 归属：shadow_pillar

### StartFX  [643–657]
- 归属：shadow_pillar_spell

### StopTask  [741–744]
- 归属：shadow_pillar_spell

### Target_DoShake  [521–523]
- 归属：shadow_pillar_target

### Target_OnLoad  [540–548]
- 归属：shadow_pillar_target

### Target_OnLoadPostPass  [550–563]
- 归属：shadow_pillar_target

### Target_OnSave  [536–538]
- 归属：shadow_pillar_target

### Target_OnSetTarget  [480–519]
- 归属：shadow_pillar_target

### Target_OnTimerDone  [417–430]
- 归属：shadow_pillar_target

### Target_SetDelay  [432–437]
- 归属：shadow_pillar_target

### Target_SetTarget  [525–534]
- 归属：shadow_pillar_target

### Target_Update  [442–478]
- 归属：shadow_pillar_target

### TryFX  [596–641]
- 归属：shadow_pillar_spell

### TryRipples  [354–368]
- 归属：shadow_pillar_base_fx

### base_fn  [375–411]
- 归属：shadow_pillar_base_fx

### onremovetarget  [486–486]
- 归属：shadow_pillar_target

### ontargetattacked  [502–513]
- 归属：shadow_pillar_target

### pillar_fn  [283–324]
- 归属：shadow_pillar

### spell_fn  [746–770]
- 归属：shadow_pillar_spell

### target_fn  [565–589]
- 归属：shadow_pillar_target

