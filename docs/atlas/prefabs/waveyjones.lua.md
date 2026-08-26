# `prefabs/waveyjones.lua`

- 扫描角色：prefabs/waveyjones.lua
- 归属变体（5 个）：waveyjones, waveyjones_arm, waveyjones_hand, waveyjones_hand_art, waveyjones_marker
## 关联

### 组件
- `components/entitytracker.lua`：waveyjones, waveyjones_marker（Direct；line 150,496）
- `components/locomotor.lua`：waveyjones_hand（Direct；line 258）
- `components/playerprox.lua`：waveyjones_hand（Direct；line 262）
- `components/sanityaura.lua`：waveyjones（Direct；line 155）
- `components/timer.lua`：waveyjones, waveyjones_hand, waveyjones_marker（Direct；line 158,267,498）
- `components/updatelooper.lua`：waveyjones, waveyjones_hand, waveyjones_marker（Direct；line 152,233,516）

### 状态图
- `stategraphs/SGwaveyjoneshand.lua`：waveyjones_hand（Direct；line 260）
- `stategraphs/SGwaveyjoneshand_art.lua`：waveyjones_hand_art（Direct；line 345）

### 大脑
- `brains/waveyjoneshandbrain.lua`：waveyjones_hand（Direct；line 269）

### 预制体依赖
- `shadowhand_fx`：waveyjones_hand（Direct；line 523）
- `waveyjones_arm`：waveyjones, waveyjones_marker（Direct；line 522,526）
- `waveyjones_hand`：waveyjones_arm（Direct；line 525）
- `waveyjones_hand_art`：waveyjones_hand（Direct；line 523）
- `waveyjones_marker`：waveyjones, waveyjones_marker（Direct；line 522,526）


## 函数

### ClearWaveyJonesTarget  [170–175]
- 归属：waveyjones_hand

### angles_sort  [402–402]
- 归属：waveyjones_marker

### armfn  [378–399]
- 归属：waveyjones_arm

### fn  [118–167]
- 归属：waveyjones

### handartfn  [316–363]
- 归属：waveyjones_hand_art

### handfn  [239–283]
- 归属：waveyjones_hand

### marker_timer_done  [460–472]
- 归属：waveyjones_marker

### marker_wallupdate  [474–479]
- 归属：waveyjones_marker

### markerfn  [481–520]
- 归属：waveyjones_marker

### periodic_fx_queue  [312–314]
- 归属：waveyjones_hand_art

### playerfar  [218–220]
- 归属：waveyjones_hand

### playernear  [214–216]
- 归属：waveyjones_hand

### playlaugh_delay  [104–108]
- 归属：waveyjones

### remove_when_scared_ends  [33–37]
- 归属：waveyjones, waveyjones_marker

### resetposition  [177–192]
- 归属：waveyjones_hand

### scarearm  [39–49]
- 归属：waveyjones, waveyjones_marker

### scareaway  [51–64]
- 归属：waveyjones, waveyjones_marker

### spawn_shadowhand_fx  [308–311]
- 归属：waveyjones_hand_art

### spawnarm_on_initialize  [76–92]
- 归属：waveyjones

### spawnjones  [403–458]
- 归属：waveyjones_marker

### test_for_scared  [66–74]
- 归属：waveyjones

### waveyjones_arm_initialize  [366–376]
- 归属：waveyjones_arm

### waveyjones_hand_initialize  [222–237]
- 归属：waveyjones_hand

### waveyjones_initialize  [94–102]
- 归属：waveyjones

### waveyjones_laugh  [109–116]
- 归属：waveyjones

