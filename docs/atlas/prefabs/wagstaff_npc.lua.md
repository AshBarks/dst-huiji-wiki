# `prefabs/wagstaff_npc.lua`

- 扫描角色：prefabs/wagstaff_npc.lua
- 归属变体（8 个）：alterguardian_contained, enable_lunar_rift_construction_container, wagstaff_npc, wagstaff_npc_finale_fx, wagstaff_npc_mutations, wagstaff_npc_pstboss, wagstaff_npc_wagpunk, wagstaff_npc_wagpunk_arena
## 关联

### 组件
- `components/constructionsite.lua`：wagstaff_npc_pstboss（Direct；line 639）
- `components/container.lua`：enable_lunar_rift_construction_container（Direct；line 1750）
- `components/hudindicatable.lua`：wagstaff_npc, wagstaff_npc_pstboss（Direct；line 467,789）
- `components/inspectable.lua`：alterguardian_contained, wagstaff_npc, wagstaff_npc_mutations, wagstaff_npc_pstboss, wagstaff_npc_wagpunk, wagstaff_npc_wagpunk_arena（Direct；line 519,808,1053,1149,1432,1718）
- `components/inventory.lua`：wagstaff_npc, wagstaff_npc_wagpunk_arena（Direct；line 490,1424）
- `components/knownlocations.lua`：wagstaff_npc, wagstaff_npc_wagpunk, wagstaff_npc_wagpunk_arena（Direct；line 498,1143,1422）
- `components/locomotor.lua`：wagstaff_npc, wagstaff_npc_pstboss, wagstaff_npc_wagpunk, wagstaff_npc_wagpunk_arena（Direct；line 485,805,1146,1429）
- `components/lootdropper.lua`：alterguardian_contained, wagstaff_npc, wagstaff_npc_wagpunk_arena（Direct；line 494,1427,1716）
- `components/npc_talker.lua`：wagstaff_npc_finale_fx, wagstaff_npc_mutations, wagstaff_npc_wagpunk_arena（Direct；line 1037,1395,1623）
- `components/playerprox.lua`：wagstaff_npc（Direct；line 510）
- `components/talker.lua`：wagstaff_npc, wagstaff_npc_finale_fx, wagstaff_npc_mutations, wagstaff_npc_pstboss, wagstaff_npc_wagpunk, wagstaff_npc_wagpunk_arena（Direct；line 458,780,1029,1120,1384,1614）
- `components/teleportedoverride.lua`：wagstaff_npc（Direct；line 531）
- `components/timer.lua`：wagstaff_npc, wagstaff_npc_pstboss, wagstaff_npc_wagpunk, wagstaff_npc_wagpunk_arena（Direct；line 515,825,1139,1418）
- `components/trader.lua`：wagstaff_npc, wagstaff_npc_pstboss, wagstaff_npc_wagpunk_arena（Direct；line 502,811,1268）
- `components/updatelooper.lua`：wagstaff_npc_finale_fx（Direct；line 1635）

### 状态图
- `stategraphs/SGwagstaff_npc.lua`：wagstaff_npc, wagstaff_npc_mutations, wagstaff_npc_pstboss, wagstaff_npc_wagpunk, wagstaff_npc_wagpunk_arena（Direct；line 528,817,1058,1153,1439）

### 预制体依赖
- `alterguardian_contained`：wagstaff_npc（Direct；line 1760）
- `enable_lunar_rift_construction_container`：wagstaff_npc_pstboss（Direct；line 1761）
- `mapscroll_tricker`：wagstaff_npc_wagpunk_arena（Direct；line 1764）
- `prefabs/gestalt_cage.lua`：wagstaff_npc_mutations, wagstaff_npc_wagpunk_arena（Direct；line 1762,1764）
- `prefabs/moonstorm_static.lua`：wagstaff_npc（Direct；line 1760）
- `prefabs/security_pulse_cage.lua`：wagstaff_npc_mutations（Direct；line 1762）
- `prefabs/wagdrone_flying.lua`：wagstaff_npc_wagpunk_arena（Direct；line 1764）
- `prefabs/wagdrone_rolling.lua`：wagstaff_npc_wagpunk_arena（Direct；line 1764）
- `wagstaff_tool_1`：wagstaff_npc（Direct；line 1760）
- `wagstaff_tool_2`：wagstaff_npc（Direct；line 1760）
- `wagstaff_tool_3`：wagstaff_npc（Direct；line 1760）
- `wagstaff_tool_4`：wagstaff_npc（Direct；line 1760）
- `wagstaff_tool_5`：wagstaff_npc（Direct；line 1760）
- `winter_ornament_boss_wagstaff`：wagstaff_npc（Direct；line 1760）


## 函数

### AddTrader_Arena  [1263–1273]
- 归属：wagstaff_npc_wagpunk_arena

### AttachToAlter  [1517–1533]
- 归属：wagstaff_npc_finale_fx

### ConstructionSite_OnConstructed  [608–616]
- 归属：wagstaff_npc_pstboss

### DoExperiment_Arena  [1199–1206]
- 归属：wagstaff_npc_wagpunk_arena

### DoFadeOutIn_Arena  [1216–1221]
- 归属：wagstaff_npc_wagpunk_arena

### DoFadeOut_Arena  [1207–1215]
- 归属：wagstaff_npc_wagpunk_arena

### DropNotesForDroneCount_Arena  [1295–1305]
- 归属：wagstaff_npc_wagpunk_arena

### EnableRiftContainerFn  [1729–1756]
- 归属：enable_lunar_rift_construction_container

### GiveGestaltCageToToss_Arena  [1285–1292]
- 归属：wagstaff_npc_wagpunk_arena

### LaunchGameItem  [149–164]
- 归属：wagstaff_npc, wagstaff_npc_mutations

### MutationsQuestFn  [987–1069]
- 归属：wagstaff_npc_mutations

### Mutations_GiveSecurityPulseCage  [871–889]
- 归属：wagstaff_npc_mutations

### Mutations_OnEntitySleep  [977–985]
- 归属：wagstaff_npc_mutations

### Mutations_OnLoad  [966–975]
- 归属：wagstaff_npc_mutations

### Mutations_TalkAboutMutatedCreature  [945–964]
- 归属：wagstaff_npc_mutations

### OnEntitySleep  [374–378]
- 归属：wagstaff_npc

### OnEntitySleep_Arena  [1275–1283]
- 归属：wagstaff_npc_wagpunk_arena

### OnFinishExperiment_maptamper  [1194–1198]
- 归属：wagstaff_npc_wagpunk_arena

### OnGetItemFromPlayer  [106–113]
- 归属：wagstaff_npc

### OnGetItemFromPlayer_Arena  [1222–1248]
- 归属：wagstaff_npc_wagpunk_arena

### OnInit_Arena  [1181–1184]
- 归属：wagstaff_npc_wagpunk_arena

### OnLoadPostPass_Arena  [1324–1331]
- 归属：wagstaff_npc_wagpunk_arena

### OnLoad_Arena  [1319–1322]
- 归属：wagstaff_npc_wagpunk_arena

### OnMusicDirty  [62–71]
- 归属：wagstaff_npc

### OnRefuseItem  [115–125]
- 归属：wagstaff_npc

### OnRefuseItem_Arena  [1250–1261]
- 归属：wagstaff_npc_wagpunk_arena

### OnRestoreItemPhysics  [145–147]
- 归属：wagstaff_npc, wagstaff_npc_mutations

### OnSave_Arena  [1315–1317]
- 归属：wagstaff_npc_wagpunk_arena

### OnTeleported  [405–414]
- 归属：wagstaff_npc

### PstBossOnLoad  [718–729]
- 归属：wagstaff_npc_pstboss

### PstBossOnSave  [712–716]
- 归属：wagstaff_npc_pstboss

### PushMusic  [56–60]
- 归属：wagstaff_npc

### ShouldAcceptItem  [100–104]
- 归属：wagstaff_npc

### ShouldAcceptItem_Arena  [1185–1192]
- 归属：wagstaff_npc_wagpunk_arena

### ShouldTrackfn  [386–392]
- 归属：wagstaff_npc, wagstaff_npc_pstboss

### ShowUp  [891–904]
- 归属：wagstaff_npc_mutations

### StartMusic  [73–78]
- 归属：wagstaff_npc

### StopMusic  [80–85]
- 归属：wagstaff_npc

### WagpunkFn  [1080–1161]
- 归属：wagstaff_npc_wagpunk

### WaitForTool  [133–143]
- 归属：wagstaff_npc

### _Mutations_TalkAboutMutatedCreature_Internal  [906–943]
- 归属：wagstaff_npc_mutations

### alterguardian_containedfn  [1685–1725]
- 归属：alterguardian_contained

### cleartasks  [367–372]
- 归属：wagstaff_npc

### contained_animover  [1666–1669]
- 归属：alterguardian_contained

### do_no_way_2  [187–190]
- 归属：wagstaff_npc

### do_no_way_erode  [184–186]
- 归属：wagstaff_npc

### do_tool_chatter  [127–130]
- 归属：wagstaff_npc

### doblueprintcheck  [207–219]
- 归属：wagstaff_npc

### docollect  [1671–1683]
- 归属：alterguardian_contained

### donpcerode  [577–590]
- 归属：wagstaff_npc_mutations, wagstaff_npc_pstboss, wagstaff_npc_wagpunk

### doplayerrequest  [652–659]
- 归属：wagstaff_npc_pstboss

### erode  [304–365]
- 归属：alterguardian_contained, wagstaff_npc, wagstaff_npc_mutations, wagstaff_npc_pstboss, wagstaff_npc_wagpunk, wagstaff_npc_wagpunk_arena

### finale_Brighten  [1486–1491]
- 归属：wagstaff_npc_finale_fx

### finale_CreateSilhouette  [1557–1578]
- 归属：wagstaff_npc_finale_fx

### finale_DoTalkSound  [1540–1547]
- 归属：wagstaff_npc_finale_fx

### finale_Materialize  [1464–1469]
- 归属：wagstaff_npc_finale_fx

### finale_OnAnimOver  [1581–1589]
- 归属：wagstaff_npc_finale_fx

### finale_OnEntityReplicated  [1505–1515]
- 归属：wagstaff_npc_finale_fx

### finale_OnRemoveEntity  [1493–1503]
- 归属：wagstaff_npc_finale_fx

### finale_PostUpdate  [1549–1555]
- 归属：wagstaff_npc_finale_fx

### finale_StopTalkSound  [1535–1538]
- 归属：wagstaff_npc_finale_fx

### finale_UpdateBrighten  [1471–1484]
- 归属：wagstaff_npc_finale_fx

### finale_UpdateMaterialize  [1450–1462]
- 归属：wagstaff_npc_finale_fx

### finale_fn  [1591–1662]
- 归属：wagstaff_npc_finale_fx

### fn  [417–574]
- 归属：wagstaff_npc

### getline  [92–98]
- 归属：wagstaff_npc

### giveblueprints  [166–182]
- 归属：wagstaff_npc

### lunar_guardian_incoming_Arena  [1307–1313]
- 归属：wagstaff_npc_wagpunk_arena

### new_met_player_chatter_1  [235–242]
- 归属：wagstaff_npc

### new_met_player_chatter_2  [228–233]
- 归属：wagstaff_npc

### new_met_player_chatter_3  [221–226]
- 归属：wagstaff_npc

### onplayernear  [244–281]
- 归属：wagstaff_npc

### ontalk  [380–382]
- 归属：wagstaff_npc, wagstaff_npc_pstboss

### ontimerdone  [285–302]
- 归属：wagstaff_npc, wagstaff_npc_wagpunk, wagstaff_npc_wagpunk_arena

### pstbossOnRefuseItem  [622–650]
- 归属：wagstaff_npc_pstboss

### pstbossShouldAcceptItem  [618–620]
- 归属：wagstaff_npc_pstboss

### pstbossfn  [731–844]
- 归属：wagstaff_npc_pstboss

### pstbossontimerdone  [704–710]
- 归属：wagstaff_npc_pstboss

### relocate_wagstaff  [669–702]
- 归属：wagstaff_npc_pstboss

### spawn_device  [592–606]
- 归属：wagstaff_npc_pstboss

### teleport_override_fn  [394–403]
- 归属：wagstaff_npc

### wagpunk_ShowUp  [1073–1078]
- 归属：wagstaff_npc_wagpunk

### wagpunk_arena_fn  [1333–1446]
- 归属：wagstaff_npc_wagpunk_arena

### waypointadvance  [191–205]
- 归属：wagstaff_npc

