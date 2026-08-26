# `prefabs/boat.lua`

- 扫描角色：prefabs/boat.lua
- 归属变体（19 个）：boat, boat_ancient, boat_ancient_item, boat_grass, boat_grass_item, boat_grass_item_collision, boat_grass_player_collision, boat_ice, boat_ice_crabking, boat_ice_deploy_blocker, boat_ice_item_collision, boat_ice_player_collision, boat_item, boat_item_collision, boat_otterden, boat_otterden_item_collision, boat_otterden_player_collision, boat_pirate, boat_player_collision
## 关联

### 组件
- `components/boatdrifter.lua`：boat, boat_ancient, boat_grass, boat_ice, boat_ice_crabking, boat_otterden, boat_pirate（Direct；line 579）
- `components/boatphysics.lua`：boat, boat_ancient, boat_grass, boat_ice, boat_ice_crabking, boat_otterden, boat_pirate（Direct；line 578）
- `components/boatring.lua`：boat, boat_ancient, boat_grass, boat_ice, boat_ice_crabking, boat_otterden, boat_pirate（Direct；line 576）
- `components/boatringdata.lua`：boat, boat_ancient, boat_grass, boat_ice, boat_ice_crabking, boat_otterden, boat_pirate（Direct；line 450）
- `components/boattrail.lua`：boat, boat_ancient, boat_grass, boat_ice, boat_ice_crabking, boat_otterden, boat_pirate（Direct；line 447）
- `components/burnable.lua`：boat_ancient_item, boat_grass_item, boat_item（HelperExpanded；line 1493）
- `components/deployable.lua`：boat_ancient_item, boat_grass_item, boat_item（Direct；line 1482）
- `components/entitytracker.lua`：boat_otterden（Direct；line 1224）
- `components/floater.lua`：boat_ancient_item, boat_grass_item, boat_item（HelperExpanded；line 1476）
- `components/fuel.lua`：boat_ancient_item, boat_grass_item, boat_item（Direct；line 1490）
- `components/hauntable.lua`：boat_ancient_item, boat_grass_item, boat_item（HelperExpanded；line 1495）
- `components/health.lua`：boat, boat_ancient, boat_grass, boat_ice, boat_ice_crabking, boat_otterden, boat_pirate（Direct；line 582）
- `components/healthsyncer.lua`：boat, boat_ancient, boat_grass, boat_ice, boat_ice_crabking, boat_otterden, boat_pirate（Direct；line 416）
- `components/hull.lua`：boat, boat_ancient, boat_grass, boat_ice, boat_ice_crabking, boat_otterden, boat_pirate（Direct；line 534）
- `components/hullhealth.lua`：boat, boat_ancient, boat_grass, boat_ice, boat_ice_crabking, boat_otterden, boat_pirate（Direct；line 577）
- `components/inspectable.lua`：boat_ancient_item, boat_grass_item, boat_item（Direct；line 1487）
- `components/inventoryitem.lua`：boat_ancient_item, boat_grass_item, boat_item（Direct；line 1488）
- `components/placer.lua`：boat, boat_ancient, boat_ancient_item, boat_grass, boat_grass_item, boat_grass_item_collision, boat_grass_player_collision, boat_ice, boat_ice_crabking, boat_ice_deploy_blocker, boat_ice_item_collision, boat_ice_player_collision, boat_item, boat_item_collision, boat_otterden, boat_otterden_item_collision, boat_otterden_player_collision, boat_pirate, boat_player_collision（HelperExpanded；line 1643,1649,1666）
- `components/propagator.lua`：boat_ancient_item, boat_grass_item, boat_item（HelperExpanded；line 1494）
- `components/repairable.lua`：boat, boat_ancient, boat_grass, boat_ice, boat_ice_crabking, boat_otterden, boat_pirate（Direct；line 572）
- `components/reticule.lua`：boat, boat_ancient, boat_grass, boat_ice, boat_ice_crabking, boat_otterden, boat_pirate（Direct；line 424）
- `components/savedrotation.lua`：boat, boat_ancient, boat_grass, boat_ice, boat_ice_crabking, boat_otterden, boat_pirate（Direct；line 580）
- `components/walkableplatform.lua`：boat, boat_ancient, boat_grass, boat_ice, boat_ice_crabking, boat_otterden, boat_pirate（Direct；line 412）
- `components/waterphysics.lua`：boat, boat_ancient, boat_grass, boat_ice, boat_ice_crabking, boat_otterden, boat_pirate（Direct；line 420）

### 状态图
- `stategraphs/SGboat.lua`：boat, boat_ancient, boat_grass, boat_ice, boat_ice_crabking, boat_otterden, boat_pirate（Direct；line 586）

### 预制体依赖
- `boat_ancient`：boat_ancient_item（Direct；line 1648）
- `boat_ancient_container`：boat_ancient（Direct；line 1647）
- `boat_grass`：boat_grass_item（Direct；line 1665）
- `boat_grass_erode`：boat_grass（Direct；line 1655）
- `boat_grass_erode_water`：boat_grass（Direct；line 1655）
- `boat_grass_item_collision`：boat, boat_pirate（Direct；line 1639,1645）
- `boat_grass_player_collision`：boat, boat_pirate（Direct；line 1639,1645）
- `boat_ice_deploy_blocker`：boat_ice, boat_ice_crabking（Direct；line 1659,1660）
- `boat_item_collision`：boat, boat_pirate（Direct；line 1639,1645）
- `boat_otterden_erode`：boat_otterden（Direct；line 1651）
- `boat_otterden_erode_water`：boat_otterden（Direct；line 1651）
- `boat_otterden_item_collision`：boat_otterden（Direct；line 1651）
- `boat_otterden_player_collision`：boat_otterden（Direct；line 1651）
- `boat_player_collision`：boat, boat_pirate（Direct；line 1639,1645）
- `boatfragment03`：boat, boat_pirate（Direct；line 1639,1645）
- `boatfragment04`：boat, boat_pirate（Direct；line 1639,1645）
- `boatfragment05`：boat, boat_pirate（Direct；line 1639,1645）
- `boatlip_ancient`：boat_ancient（Direct；line 1647）
- `boatlip_grass`：boat_grass（Direct；line 1655）
- `boatlip_ice`：boat_ice, boat_ice_crabking（Direct；line 1659,1660）
- `boatlip_otterden`：boat_otterden（Direct；line 1651）
- `degrade_fx_grass`：boat_grass（Direct；line 1655）
- `degrade_fx_ice`：boat_ice, boat_ice_crabking（Direct；line 1659,1660）
- `fx_boat_crackle`：boat, boat_pirate（Direct；line 1639,1645）
- `fx_boat_pop`：boat, boat_pirate（Direct；line 1639,1645）
- `fx_grass_boat_fluff`：boat_grass, boat_otterden（Direct；line 1651,1655）
- `prefabs/boat.lua`：boat_item（Direct；line 1642）
- `prefabs/boat_leak.lua`：boat, boat_pirate（Direct；line 1639,1645）
- `prefabs/boat_water_fx.lua`：boat, boat_pirate（Direct；line 1639,1645）
- `prefabs/boatlip.lua`：boat, boat_pirate（Direct；line 1639,1645）
- `prefabs/burnable_locator_medium.lua`：boat, boat_pirate（Direct；line 1639,1645）
- `prefabs/mast.lua`：boat, boat_pirate（Direct；line 1639,1645）
- `prefabs/otterden.lua`：boat_otterden（Direct；line 1651）
- `prefabs/rudder.lua`：boat, boat_pirate（Direct；line 1639,1645）
- `prefabs/steeringwheel.lua`：boat, boat_pirate（Direct；line 1639,1645）
- `prefabs/walkingplank.lua`：boat, boat_pirate（Direct；line 1639,1645）
- `walkingplank_ancient`：boat_ancient（Direct；line 1647）
- `walkingplank_grass`：boat, boat_pirate（Direct；line 1639,1645）


## 函数

### AddConstrainedPhysicsObj  [248–252]
- 归属：boat, boat_ancient, boat_grass, boat_ice, boat_ice_crabking, boat_otterden, boat_pirate

### CLIENT_CanDeployBoat  [1439–1456]
- 归属：boat_ancient_item, boat_grass_item, boat_item

### CLIENT_MakeOtterdenTuft  [1149–1165]
- 归属：boat_otterden

### DisableBoatItemCollision  [307–312]
- 归属：boat, boat_ancient, boat_grass, boat_ice, boat_ice_crabking, boat_otterden, boat_pirate

### EnableBoatItemCollision  [300–305]
- 归属：boat, boat_ancient, boat_grass, boat_ice, boat_ice_crabking, boat_otterden, boat_pirate

### GetSafePhysicsRadius  [482–484]
- 归属：boat, boat_ancient, boat_grass, boat_ice, boat_ice_crabking, boat_otterden, boat_pirate

### InstantlyBreakBoat  [463–480]
- 归属：boat, boat_ancient, boat_grass, boat_ice, boat_ice_crabking, boat_otterden, boat_pirate

### IsBoatEdgeOverLand  [486–516]
- 归属：boat, boat_ancient, boat_grass, boat_ice, boat_ice_crabking, boat_otterden, boat_pirate

### OnEntityReplicated  [360–364]
- 归属：boat, boat_ancient, boat_grass, boat_ice, boat_ice_crabking, boat_otterden, boat_pirate

### OnLoadPostPass  [165–186]
- 归属：boat, boat_ancient, boat_grass, boat_ice, boat_ice_crabking, boat_otterden, boat_pirate

### OnPhysicsSleep  [329–333]
- 归属：boat, boat_ancient, boat_grass, boat_ice, boat_ice_crabking, boat_otterden, boat_pirate

### OnPhysicsWake  [314–323]
- 归属：boat, boat_ancient, boat_grass, boat_ice, boat_ice_crabking, boat_otterden, boat_pirate

### OnRowed_OtterDen  [1025–1032]
- 归属：boat_otterden

### OnSpawnNewBoatLeak  [188–213]
- 归属：boat, boat_ancient, boat_ice, boat_ice_crabking, boat_pirate

### OnSpawnNewBoatLeak_Grass  [215–233]
- 归属：boat_grass, boat_otterden

### RemoveConstrainedPhysicsObj  [235–240]
- 归属：boat, boat_ancient, boat_grass, boat_ice, boat_ice_crabking, boat_otterden, boat_pirate

### ReticuleTargetFn  [267–298]
- 归属：boat, boat_ancient, boat_grass, boat_ice, boat_ice_crabking, boat_otterden, boat_pirate

### SpawnFragment  [345–358]
- 归属：boat, boat_ancient, boat_grass, boat_ice, boat_ice_crabking, boat_otterden, boat_pirate

### StartBoatPhysics  [341–343]
- 归属：boat, boat_ancient, boat_grass, boat_ice, boat_ice_crabking, boat_otterden, boat_pirate

### StopBoatPhysics  [335–339]
- 归属：boat, boat_ancient, boat_grass, boat_ice, boat_ice_crabking, boat_otterden, boat_pirate

### _check_placer_offset  [1610–1617]
- 归属：（未归属）

### _set_placer_layer  [1604–1608]
- 归属：（未归属）

### ancient_fn  [956–1022]
- 归属：boat_ancient

### ancient_item_fn  [1546–1566]
- 归属：boat_ancient_item

### ancient_ondeploy  [1539–1544]
- 归属：boat_ancient_item

### ancient_placer_postinit  [1625–1630]
- 归属：（未归属）

### boat_grass_item_collision_fn  [1582–1584]
- 归属：boat_grass_item_collision

### boat_grass_player_collision_fn  [1578–1580]
- 归属：boat_grass_player_collision

### boat_ice_item_collision_fn  [1591–1593]
- 归属：boat_ice_item_collision

### boat_ice_player_collision_fn  [1587–1589]
- 归属：boat_ice_player_collision

### boat_item_collision_fn  [1573–1575]
- 归属：boat_item_collision

### boat_item_collision_template  [685–720]
- 归属：boat_grass_item_collision, boat_ice_item_collision, boat_item_collision, boat_otterden_item_collision

### boat_otterden_item_collision_fn  [1599–1601]
- 归属：boat_otterden_item_collision

### boat_otterden_player_collision_fn  [1595–1597]
- 归属：boat_otterden_player_collision

### boat_player_collision_fn  [1569–1571]
- 归属：boat_player_collision

### boat_player_collision_template  [652–683]
- 归属：boat_grass_player_collision, boat_ice_player_collision, boat_otterden_player_collision, boat_player_collision

### build_boat_collision_mesh  [604–650]
- 归属：boat_grass_item_collision, boat_grass_player_collision, boat_ice_item_collision, boat_ice_player_collision, boat_item_collision, boat_otterden_item_collision, boat_otterden_player_collision, boat_player_collision

### common_item_fn_pre  [1458–1479]
- 归属：boat_ancient_item, boat_grass_item, boat_item

### common_item_fn_pst  [1481–1498]
- 归属：boat_ancient_item, boat_grass_item, boat_item

### constrain_object_to_boat  [242–247]
- 归属：boat, boat_ancient, boat_grass, boat_ice, boat_ice_crabking, boat_otterden, boat_pirate

### create_common_pre  [366–459]
- 归属：boat, boat_ancient, boat_grass, boat_ice, boat_ice_crabking, boat_otterden, boat_pirate

### create_master_pst  [519–602]
- 归属：boat, boat_ancient, boat_grass, boat_ice, boat_ice_crabking, boat_otterden, boat_pirate

### do_boat_container_offset  [948–952]
- 归属：boat_ancient

### empty_loot_function  [461–461]
- 归属：boat, boat_ancient, boat_grass, boat_ice, boat_ice_crabking, boat_otterden, boat_pirate

### grass_fn  [806–870]
- 归属：boat_grass

### grass_item_fn  [1517–1537]
- 归属：boat_grass_item

### grass_placer_postinit  [1632–1636]
- 归属：（未归属）

### ice_crabking_fn  [1408–1435]
- 归属：boat_ice_crabking

### ice_floe_deploy_blocker_fn  [1275–1292]
- 归属：boat_ice_deploy_blocker

### ice_fn  [1306–1406]
- 归属：boat_ice, boat_ice_crabking

### ice_ondeath  [1294–1304]
- 归属：boat_ice, boat_ice_crabking

### item_fn  [1500–1515]
- 归属：boat_item

### on_dead_otterden_added  [1114–1119]
- 归属：boat_otterden

### on_start_steering  [254–258]
- 归属：boat, boat_ancient, boat_grass, boat_ice, boat_ice_crabking, boat_otterden, boat_pirate

### on_stop_steering  [260–265]
- 归属：boat, boat_ancient, boat_grass, boat_ice, boat_ice_crabking, boat_otterden, boat_pirate

### oncannonremoved  [876–878]
- 归属：boat_pirate

### ondeploy  [722–749]
- 归属：boat_ancient_item, boat_grass_item, boat_item

### otterden_comment_timeout  [1034–1036]
- 归属：boat_otterden

### otterden_fn  [1167–1272]
- 归属：boat_otterden

### otterden_initialize  [1102–1112]
- 归属：boat_otterden

### otterden_on_update  [1067–1100]
- 归属：boat_otterden

### otterden_onload  [1131–1138]
- 归属：boat_otterden

### otterden_onloadpostpass  [1140–1147]
- 归属：boat_otterden

### otterden_onsave  [1121–1129]
- 归属：boat_otterden

### otterden_start_erosion  [1037–1052]
- 归属：boat_otterden

### otterden_stop_erosion  [1053–1066]
- 归属：boat_otterden

### physicssleep_stopupdating  [325–328]
- 归属：boat, boat_ancient, boat_grass, boat_ice, boat_ice_crabking, boat_otterden, boat_pirate

### pirate_fn  [889–946]
- 归属：boat_pirate

### pirate_initialize  [873–887]
- 归属：boat_pirate

### sinkloot  [1415–1429]
- 归属：boat_ice_crabking

### wood_fn  [751–804]
- 归属：boat

### wood_placer_postinit  [1619–1623]
- 归属：（未归属）

