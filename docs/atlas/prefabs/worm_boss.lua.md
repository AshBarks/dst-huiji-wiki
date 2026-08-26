# `prefabs/worm_boss.lua`

- 扫描角色：prefabs/worm_boss.lua
- 归属变体（6 个）：worm_boss, worm_boss_dirt, worm_boss_dirt_ground_fx, worm_boss_head, worm_boss_segment, worm_boss_tail
## 关联

### 组件
- `components/colouradder.lua`：worm_boss_dirt（Direct；line 1253）
- `components/combat.lua`：worm_boss, worm_boss_dirt（Direct；line 610,1258）
- `components/groundpounder.lua`：worm_boss_dirt（Direct；line 1263）
- `components/health.lua`：worm_boss, worm_boss_dirt（Direct；line 606,1255）
- `components/highlightchild.lua`：worm_boss_dirt（Direct；line 1240）
- `components/inspectable.lua`：worm_boss_dirt（Direct；line 1252）
- `components/inventory.lua`：worm_boss（Direct；line 603）
- `components/lootdropper.lua`：worm_boss, worm_boss_head, worm_boss_segment, worm_boss_tail（Direct；line 600,762,806,1040）
- `components/sanityaura.lua`：worm_boss_dirt（Direct；line 1274）
- `components/timer.lua`：worm_boss（Direct；line 598）
- `components/updatelooper.lua`：worm_boss, worm_boss_segment（Direct；line 615,1023）

### 状态图
- `stategraphs/SGworm_boss_head.lua`：worm_boss_head（Direct；line 765）
- `stategraphs/SGworm_boss_tail.lua`：worm_boss_tail（Direct；line 808）

### 预制体依赖
- `chesspiece_wormboss_sketch`：worm_boss, worm_boss_dirt, worm_boss_dirt_ground_fx, worm_boss_head, worm_boss_segment, worm_boss_tail（Direct；line 1324,1325,1326,1327,1328,1329）
- `winter_ornament_boss_wormboss`：worm_boss, worm_boss_dirt, worm_boss_dirt_ground_fx, worm_boss_head, worm_boss_segment, worm_boss_tail（Direct；line 1324,1325,1326,1327,1328,1329）
- `worm_boss_dirt`：worm_boss, worm_boss_dirt, worm_boss_dirt_ground_fx, worm_boss_head, worm_boss_segment, worm_boss_tail（Direct；line 1324,1325,1326,1327,1328,1329）
- `worm_boss_dirt_ground_fx`：worm_boss, worm_boss_dirt, worm_boss_dirt_ground_fx, worm_boss_head, worm_boss_segment, worm_boss_tail（Direct；line 1324,1325,1326,1327,1328,1329）
- `worm_boss_head`：worm_boss, worm_boss_dirt, worm_boss_dirt_ground_fx, worm_boss_head, worm_boss_segment, worm_boss_tail（Direct；line 1324,1325,1326,1327,1328,1329）
- `worm_boss_segment`：worm_boss, worm_boss_dirt, worm_boss_dirt_ground_fx, worm_boss_head, worm_boss_segment, worm_boss_tail（Direct；line 1324,1325,1326,1327,1328,1329）


## 函数

### AddHighlightHandler  [708–727]
- 归属：worm_boss_head, worm_boss_segment, worm_boss_tail

### CLIENT_Segment_OnUpdate  [913–961]
- 归属：worm_boss_segment

### CalcSanityAura  [1206–1211]
- 归属：worm_boss_dirt

### DeserializePosition  [292–294]
- 归属：worm_boss

### Dirt_DamageRedirectFn  [1150–1176]
- 归属：worm_boss_dirt

### Dirt_EmergeHead  [1051–1059]
- 归属：worm_boss_dirt

### Dirt_OnAnimOver  [1061–1091]
- 归属：worm_boss_dirt

### Dirt_OnAttacked  [1130–1142]
- 归属：worm_boss_dirt

### Dirt_OnElectrocute  [1144–1148]
- 归属：worm_boss_dirt

### DoElectrocute  [1093–1128]
- 归属：worm_boss_dirt

### DoThornDamage  [823–847]
- 归属：worm_boss_segment

### GenerateLoot  [42–77]
- 归属：worm_boss, worm_boss_segment

### HighlightHandler_OnRemoveEntity  [645–655]
- 归属：worm_boss_head, worm_boss_segment, worm_boss_tail

### HighlightHandler_SetOwner  [669–697]
- 归属：worm_boss_head, worm_boss_segment, worm_boss_tail

### KeepTargetFn  [102–111]
- 归属：worm_boss

### NewTarget  [113–117]
- 归属：worm_boss

### OnAttacked  [119–129]
- 归属：worm_boss

### OnDeath  [180–237]
- 归属：worm_boss

### OnDeathEnded  [247–284]
- 归属：worm_boss

### OnDirtPositionDirty  [967–970]
- 归属：worm_boss_segment

### OnHitEvent  [972–974]
- 归属：worm_boss_segment

### OnLoad  [342–396]
- 归属：worm_boss

### OnLoadPostPass  [399–422]
- 归属：worm_boss

### OnRemoveEntity  [432–475]
- 归属：worm_boss

### OnSave  [296–340]
- 归属：worm_boss

### OnSegTimeDirty  [963–965]
- 归属：worm_boss_segment

### OnSetHighlightOwners  [699–706]
- 归属：worm_boss_head, worm_boss_segment, worm_boss_tail

### OnSyncOwnerDirty  [641–643]
- 归属：worm_boss_head, worm_boss_segment, worm_boss_tail

### OnUpdate  [131–139]
- 归属：worm_boss

### ProcessThornDamage  [146–176]
- 归属：worm_boss_dirt, worm_boss_segment

### PushMusic  [538–547]
- 归属：worm_boss

### RetargetFn  [87–100]
- 归属：worm_boss

### Segment_OnAnimOver  [884–904]
- 归属：worm_boss_segment

### Segment_Restart  [851–882]
- 归属：worm_boss_segment

### Segment_UpdatePredictionData  [906–909]
- 归属：worm_boss_segment

### SerializePosition  [288–290]
- 归属：worm_boss

### SetHighlightOwners  [657–667]
- 归属：worm_boss_head, worm_boss_segment, worm_boss_tail

### SetState  [426–430]
- 归属：worm_boss

### Worm_GetSegmentFromPool  [519–529]
- 归属：worm_boss

### Worm_OnEntitySleep  [497–506]
- 归属：worm_boss

### Worm_OnEntityWake  [508–515]
- 归属：worm_boss

### Worm_ReturnSegmentToPool  [531–536]
- 归属：worm_boss

### Worm_TestForRemoval  [479–495]
- 归属：worm_boss

### _PlayDirstPstSlowAnim  [239–243]
- 归属：worm_boss

### dirt_ground_fx_fn  [1291–1321]
- 归属：worm_boss_dirt_ground_fx

### dirt_playanimation  [1178–1204]
- 归属：worm_boss_dirt

### dirtfn  [1213–1287]
- 归属：worm_boss_dirt

### fn  [554–637]
- 归属：worm_boss

### headfn  [729–770]
- 归属：worm_boss_head

### hounded_overridelocation  [548–551]
- 归属：worm_boss

### segmentfn  [976–1047]
- 归属：worm_boss_segment

### tailfn  [774–813]
- 归属：worm_boss_tail

