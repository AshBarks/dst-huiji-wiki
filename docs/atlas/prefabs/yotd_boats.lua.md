# `prefabs/yotd_boats.lua`

- 扫描角色：prefabs/yotd_boats.lua
- 归属变体（7 个）：dragonboat_body, dragonboat_item_collision, dragonboat_kit, dragonboat_pack, dragonboat_player_collision, dragonboat_shadowboat, dragonboat_shadowboat_deploy_blocker
## 关联

### 组件
- `components/boatdrifter.lua`：dragonboat_body, dragonboat_shadowboat（Direct；line 461）
- `components/boatphysics.lua`：dragonboat_body, dragonboat_shadowboat（Direct；line 460）
- `components/boatracecrew.lua`：dragonboat_shadowboat（Direct；line 607）
- `components/boatring.lua`：dragonboat_body, dragonboat_shadowboat（Direct；line 452）
- `components/boatringdata.lua`：dragonboat_body, dragonboat_shadowboat（Direct；line 198）
- `components/boattrail.lua`：dragonboat_body, dragonboat_shadowboat（Direct；line 195）
- `components/burnable.lua`：dragonboat_kit, dragonboat_pack（HelperExpanded；line 717）
- `components/crewmember.lua`：dragonboat_shadowboat（Direct；line 563）
- `components/deployable.lua`：dragonboat_kit, dragonboat_pack（Direct；line 698）
- `components/floater.lua`：dragonboat_kit, dragonboat_pack（HelperExpanded；line 686）
- `components/fuel.lua`：dragonboat_kit, dragonboat_pack（Direct；line 704）
- `components/hauntable.lua`：dragonboat_kit, dragonboat_pack（HelperExpanded；line 719）
- `components/health.lua`：dragonboat_body, dragonboat_shadowboat（Direct；line 465）
- `components/healthsyncer.lua`：dragonboat_body, dragonboat_shadowboat（Direct；line 174）
- `components/hull.lua`：dragonboat_body, dragonboat_shadowboat（Direct；line 403）
- `components/hullhealth.lua`：dragonboat_body, dragonboat_shadowboat（Direct；line 455）
- `components/inspectable.lua`：dragonboat_kit, dragonboat_pack（Direct；line 708）
- `components/inventoryitem.lua`：dragonboat_kit, dragonboat_pack（Direct；line 711）
- `components/placer.lua`：dragonboat_body, dragonboat_item_collision, dragonboat_kit, dragonboat_pack, dragonboat_player_collision, dragonboat_shadowboat, dragonboat_shadowboat_deploy_blocker（HelperExpanded；line 917,920）
- `components/platformhopdelay.lua`：dragonboat_shadowboat（Direct；line 596）
- `components/propagator.lua`：dragonboat_kit, dragonboat_pack（HelperExpanded；line 718）
- `components/repairable.lua`：dragonboat_body, dragonboat_shadowboat（Direct；line 448）
- `components/reticule.lua`：dragonboat_body, dragonboat_shadowboat（Direct；line 182）
- `components/savedrotation.lua`：dragonboat_body, dragonboat_shadowboat（Direct；line 462）
- `components/walkableplatform.lua`：dragonboat_body, dragonboat_shadowboat（Direct；line 169）
- `components/waterphysics.lua`：dragonboat_body, dragonboat_shadowboat（Direct；line 178）

### 状态图
- `stategraphs/SGboat.lua`：dragonboat_body, dragonboat_shadowboat（Direct；line 470）

### 预制体依赖
- `boat_bumper_yotd`：dragonboat_pack（Direct；line 919）
- `dragonboat_body`：dragonboat_kit, dragonboat_pack（Direct；line 916,919）
- `dragonboat_item_collision`：dragonboat_body（Direct；line 915）
- `dragonboat_player_collision`：dragonboat_body（Direct；line 915）
- `dragonboat_shadowboat_deploy_blocker`：dragonboat_shadowboat（Direct；line 922）
- `mast_yotd`：dragonboat_pack（Direct；line 919）
- `prefabs/boatrace_primemate.lua`：dragonboat_shadowboat（Direct；line 922）
- `walkingplank_yotd`：dragonboat_body, dragonboat_shadowboat（Direct；line 915,922）
- `yotd_anchor`：dragonboat_pack（Direct；line 919）
- `yotd_oar`：dragonboat_pack（Direct；line 919）
- `yotd_steeringwheel`：dragonboat_pack（Direct；line 919）


## 函数

### CLIENT_CanDeployDragonBoat  [619–636]
- 归属：dragonboat_kit, dragonboat_pack

### DisableBoatItemCollision  [233–238]
- 归属：dragonboat_body, dragonboat_shadowboat

### EnableBoatItemCollision  [222–231]
- 归属：dragonboat_body, dragonboat_shadowboat

### GetSafePhysicsRadius  [349–352]
- 归属：dragonboat_body, dragonboat_shadowboat

### InstantlyBreakBoat  [326–347]
- 归属：dragonboat_body, dragonboat_shadowboat

### IsBoatEdgeOverLand  [354–384]
- 归属：dragonboat_body, dragonboat_shadowboat

### OnBodyLoad  [529–534]
- 归属：dragonboat_body

### OnEntityReplicated  [118–122]
- 归属：dragonboat_body, dragonboat_shadowboat

### OnLoadPostPass  [271–294]
- 归属：dragonboat_body, dragonboat_shadowboat

### OnPhysicsSleep  [252–256]
- 归属：dragonboat_body, dragonboat_shadowboat

### OnPhysicsWake  [258–267]
- 归属：dragonboat_body, dragonboat_shadowboat

### OnShadowboatLoad  [578–583]
- 归属：dragonboat_shadowboat

### OnSpawnNewBoatLeak  [299–324]
- 归属：dragonboat_body, dragonboat_shadowboat

### RemoveConstrainedPhysicsObject  [210–215]
- 归属：dragonboat_body, dragonboat_shadowboat

### ReticuleTargetFn  [67–97]
- 归属：dragonboat_body, dragonboat_shadowboat

### StartBoatPhysics  [240–242]
- 归属：dragonboat_body, dragonboat_shadowboat

### StopBoatPhysics  [244–246]
- 归属：dragonboat_body, dragonboat_shadowboat

### boat_placer_postinit  [739–751]
- 归属：（未归属）

### body_fn  [536–557]
- 归属：dragonboat_body

### build_boat_collision_mesh  [774–817]
- 归属：dragonboat_item_collision, dragonboat_player_collision

### constrain_boat_item_collision  [217–220]
- 归属：dragonboat_body, dragonboat_shadowboat

### dragonboat_common  [124–207]
- 归属：dragonboat_body, dragonboat_shadowboat

### dragonboat_item_collision_fn  [854–893]
- 归属：dragonboat_item_collision

### dragonboat_player_collision_fn  [819–851]
- 归属：dragonboat_player_collision

### dragonboat_server  [386–486]
- 归属：dragonboat_body, dragonboat_shadowboat

### empty_loot_function  [297–297]
- 归属：dragonboat_body, dragonboat_shadowboat

### end_steering_reticule  [108–116]
- 归属：dragonboat_body, dragonboat_shadowboat

### item_base_fn  [665–727]
- 归属：dragonboat_kit, dragonboat_pack

### item_fn  [735–737]
- 归属：dragonboat_kit

### on_dragonboat_kit_deployed  [638–663]
- 归属：dragonboat_kit, dragonboat_pack

### pack_fn  [759–769]
- 归属：dragonboat_pack

### shadowboat_deploy_blocker_fn  [896–912]
- 归属：dragonboat_shadowboat_deploy_blocker

### shadowboat_fn  [585–614]
- 归属：dragonboat_shadowboat

### spawn_ai_captain  [560–566]
- 归属：dragonboat_shadowboat

### spawn_boat_pack_pieces  [488–527]
- 归属：dragonboat_kit, dragonboat_pack

### spawn_shadowboat_pieces  [568–576]
- 归属：dragonboat_shadowboat

### start_steering_reticule  [99–106]
- 归属：dragonboat_body, dragonboat_shadowboat

### stop_updating_callback  [248–251]
- 归属：dragonboat_body, dragonboat_shadowboat

