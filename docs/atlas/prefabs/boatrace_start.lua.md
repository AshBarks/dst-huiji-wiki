# `prefabs/boatrace_start.lua`

- 扫描角色：prefabs/boatrace_start.lua
- 归属变体（4 个）：boatrace_fireworks, boatrace_start, boatrace_start_bobber, boatrace_start_flag
## 关联

### 组件
- `components/activatable.lua`：boatrace_start（Direct；line 878）
- `components/boatrace_proximitychecker.lua`：boatrace_start（Direct；line 882）
- `components/complexprojectile.lua`：boatrace_start（Direct；line 561）
- `components/hauntable.lua`：boatrace_start（HelperExpanded；line 905）
- `components/highlightchild.lua`：boatrace_fireworks, boatrace_start_flag（Direct；line 1059,1082）
- `components/inspectable.lua`：boatrace_start（Direct；line 886）
- `components/lootdropper.lua`：boatrace_start（Direct；line 892）
- `components/talker.lua`：boatrace_start（Direct；line 889）
- `components/timer.lua`：boatrace_start（Direct；line 896）
- `components/updatelooper.lua`：boatrace_start（Direct；line 899）
- `components/waterphysics.lua`：boatrace_start（HelperExpanded；line 837）
- `components/workable.lua`：boatrace_start（Direct；line 702）

### 状态图
- `stategraphs/SGboatrace_start.lua`：boatrace_start（Direct；line 869）

### 预制体依赖
- `boatrace_fireworks`：boatrace_start（Direct；line 1091）
- `boatrace_start_bobber`：boatrace_start（Direct；line 1091）
- `boatrace_start_flag`：boatrace_start（Direct；line 1091）
- `dragonboat_shadowboat`：boatrace_start（Direct；line 1091）
- `prefabs/boatrace_checkpoint_indicator.lua`：boatrace_start（Direct；line 1091）
- `prefabs/boatrace_spectator_dragonling.lua`：boatrace_start（Direct；line 1091）
- `redpouch_yotd`：boatrace_start（Direct；line 1091）


## 函数

### CLIENT_CreateClientBobber  [750–782]
- 归属：（未归属）

### CLIENT_DeployBobber  [784–789]
- 归属：（未归属）

### CLIENT_IsNearCheckpoint  [978–980]
- 归属：（未归属）

### CLIENT_OnInit  [816–822]
- 归属：boatrace_start

### CLIENT_UpdateReticuleStartRing  [986–1002]
- 归属：（未归属）

### CreateBobberRing  [801–814]
- 归属：boatrace_start

### Flag_AnimOverBehaviour  [627–631]
- 归属：boatrace_start_flag

### GetBeacons  [480–482]
- 归属：boatrace_start

### GetCheckpoints  [476–478]
- 归属：boatrace_start

### LaunchProjectile  [33–50]
- 归属：boatrace_start

### OnActivated  [495–530]
- 归属：boatrace_start

### OnBeaconAtStartpoint  [673–685]
- 归属：boatrace_start

### OnBuilt  [109–112]
- 归属：boatrace_start

### OnCheckpointReached  [533–541]
- 归属：boatrace_start

### OnLoad  [728–732]
- 归属：boatrace_start

### OnLoadPostPass  [734–747]
- 归属：boatrace_start

### OnLootPrefabSpawned  [695–699]
- 归属：boatrace_start

### OnSave  [710–726]
- 归属：boatrace_start

### OnTimerDone  [687–693]
- 归属：boatrace_start

### OnWorkFinished  [59–67]
- 归属：boatrace_start

### OnWorked  [53–57]
- 归属：boatrace_start

### SERVER_DeployBobber  [791–797]
- 归属：boatrace_start

### automatic_checkpoint_swimmable_offset_test  [70–72]
- 归属：boatrace_start

### beaconremoved  [240–250]
- 归属：boatrace_start

### bobberfn  [945–973]
- 归属：boatrace_start_bobber

### do_automatic_checkpoint_spawn  [73–107]
- 归属：boatrace_start

### do_event_finish  [196–198]
- 归属：boatrace_start

### do_event_start  [398–474]
- 归属：boatrace_start

### do_set_placing  [657–671]
- 归属：boatrace_start

### do_spawn_prize_pouch  [556–571]
- 归属：boatrace_start

### fireworksfn  [1068–1089]
- 归属：boatrace_fireworks

### flagfn  [1045–1066]
- 归属：boatrace_start_flag

### fn  [825–942]
- 归属：boatrace_start
- 掉落锚点：893:boatrace_start_throwable_deploykit

### fuseoffOver  [490–493]
- 归属：boatrace_start

### fuseonOver  [485–488]
- 归属：boatrace_start

### gathercheckpoints  [349–396]
- 归属：boatrace_start

### getprizes  [115–125]
- 归属：boatrace_start

### kit_onload  [1007–1009]
- 归属：（未归属）

### kit_onsave  [1004–1006]
- 归属：（未归属）

### on_ai_prize_hit  [550–554]
- 归属：boatrace_start

### on_deploy_product  [982–984]
- 归属：（未归属）

### prizeOver  [200–218]
- 归属：boatrace_start

### reset_boatrace  [127–179]
- 归属：boatrace_start

### setWorkable  [701–707]
- 归属：boatrace_start

### setflag  [634–655]
- 归属：boatrace_start

### shadow_boat_Remove  [332–347]
- 归属：boatrace_start

### spawn_shadowboat  [221–238]
- 归属：boatrace_start

### testforendofrace  [544–548]
- 归属：boatrace_start

### testforwinpresentation  [181–194]
- 归属：boatrace_start

### updateloop  [253–330]
- 归属：boatrace_start

### winOver  [573–625]
- 归属：boatrace_start

