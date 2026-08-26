# `prefabs/fused_shadeling_bomb.lua`

- 扫描角色：prefabs/fused_shadeling_bomb.lua
- 归属变体（4 个）：fused_shadeling_bomb, fused_shadeling_bomb_death_fx, fused_shadeling_bomb_scorch, fused_shadeling_quickfuse_bomb
## 关联

### 组件
- `components/entitytracker.lua`：fused_shadeling_bomb（Direct；line 262）
- `components/groundshadowhandler.lua`：fused_shadeling_quickfuse_bomb（Direct；line 334）
- `components/inspectable.lua`：fused_shadeling_bomb, fused_shadeling_quickfuse_bomb（Direct；line 259,357）
- `components/locomotor.lua`：fused_shadeling_bomb（Direct；line 265）
- `components/timer.lua`：fused_shadeling_bomb（Direct；line 271）

### 状态图
- `stategraphs/SGfused_shadeling_bomb.lua`：fused_shadeling_bomb（Direct；line 284）

### 预制体依赖
- `fused_shadeling_bomb_death_fx`：fused_shadeling_bomb（Direct；line 445）
- `fused_shadeling_bomb_scorch`：fused_shadeling_bomb（Direct；line 445）
- `round_puff_fx_sm`：fused_shadeling_bomb（Direct；line 445）

### 生成引用
- `fused_shadeling_bomb_death_fx`：fused_shadeling_bomb, fused_shadeling_quickfuse_bomb（Direct；line 99）
- `fused_shadeling_bomb_scorch`：fused_shadeling_bomb, fused_shadeling_quickfuse_bomb（Direct；line 105）
- `fused_shadeling_quickfuse_bomb`：fused_shadeling_bomb（Direct；line 109）


## 函数

### Scorch_OnFadeDirty  [377–393]
- 归属：fused_shadeling_bomb_scorch

### Scorch_OnUpdateFade  [395–405]
- 归属：fused_shadeling_bomb_scorch

### ball_explode  [22–24]
- 归属：fused_shadeling_bomb

### ball_start_growing  [18–20]
- 归属：fused_shadeling_bomb

### death_fx_fn  [290–313]
- 归属：fused_shadeling_bomb_death_fx

### do_ball_grow  [50–55]
- 归属：fused_shadeling_bomb

### do_chase_tick  [145–174]
- 归属：fused_shadeling_bomb

### do_explosion_effect  [95–106]
- 归属：fused_shadeling_bomb, fused_shadeling_quickfuse_bomb

### do_full_explode  [123–140]
- 归属：fused_shadeling_bomb

### do_quick_explode  [316–320]
- 归属：fused_shadeling_quickfuse_bomb

### do_quickfuse_bomb_toss  [108–119]
- 归属：fused_shadeling_bomb

### fn  [209–287]
- 归属：fused_shadeling_bomb

### make_ball  [26–48]
- 归属：fused_shadeling_bomb

### on_load_postpass  [199–207]
- 归属：fused_shadeling_bomb

### on_spawn_delay_finished  [81–83]
- 归属：fused_shadeling_bomb

### on_spawn_finished  [74–79]
- 归属：fused_shadeling_bomb

### on_target_set  [85–90]
- 归属：fused_shadeling_bomb

### on_timer_done  [176–196]
- 归属：fused_shadeling_bomb

### quickfuse_fn  [323–366]
- 归属：fused_shadeling_quickfuse_bomb

### scorchfn  [407–443]
- 归属：fused_shadeling_bomb_scorch

