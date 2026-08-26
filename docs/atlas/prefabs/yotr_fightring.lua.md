# `prefabs/yotr_fightring.lua`

- 扫描角色：prefabs/yotr_fightring.lua
- 归属变体（4 个）：yotr_fightring, yotr_fightring_bell, yotr_fightring_kit, yotr_fightring_torch
## 关联

### 组件
- `components/activatable.lua`：yotr_fightring_bell（Direct；line 987）
- `components/burnable.lua`：yotr_fightring_kit（HelperExpanded；line 786）
- `components/deployable.lua`：yotr_fightring_kit（Direct；line 771）
- `components/entitytracker.lua`：yotr_fightring（Direct；line 653）
- `components/floater.lua`：yotr_fightring_kit（HelperExpanded；line 760）
- `components/fuel.lua`：yotr_fightring_kit（Direct；line 782）
- `components/hauntable.lua`：yotr_fightring_kit（HelperExpanded；line 790）
- `components/inspectable.lua`：yotr_fightring_bell, yotr_fightring_kit（Direct；line 776,991）
- `components/inventoryitem.lua`：yotr_fightring_kit（Direct；line 779）
- `components/lootdropper.lua`：yotr_fightring_bell（Direct；line 1002）
- `components/minigame.lua`：yotr_fightring（Direct；line 645）
- `components/placer.lua`：yotr_fightring, yotr_fightring_bell, yotr_fightring_kit, yotr_fightring_torch（HelperExpanded；line 1109）
- `components/propagator.lua`：yotr_fightring_kit（HelperExpanded；line 787）
- `components/workable.lua`：yotr_fightring_bell（Direct；line 995）

### 预制体依赖
- `lucky_goldnugget`：yotr_fightring（Direct；line 1105）
- `pillowfight_confetti_fx`：yotr_fightring（Direct；line 1105）
- `prefabs/goldnugget.lua`：yotr_fightring（Direct；line 1105）
- `prefabs/torchfire.lua`：yotr_fightring_torch（Direct；line 1107）
- `prefabs/torchfire_yotrpillowfight.lua`：yotr_fightring_torch（Direct；line 1107）
- `prefabs/yotr_fightring.lua`：yotr_fightring_kit（Direct；line 1106）
- `rabbit_confetti_fx`：yotr_fightring（Direct；line 1105）
- `yotr_fightring_bell`：yotr_fightring（Direct；line 1105）
- `yotr_fightring_torch`：yotr_fightring（Direct；line 1105）


## 函数

### AddFighterToWaitQueue  [436–450]
- 归属：yotr_fightring

### CLIENT_CanDeployFightRing  [740–742]
- 归属：yotr_fightring_kit

### CanDeployFightRingAtPoint  [34–52]
- 归属：yotr_fightring_kit

### CreatePlacerBell  [1051–1075]
- 归属：（未归属）

### CreatePlacerTorch  [1025–1049]
- 归属：（未归属）

### EnableCameraFocus  [97–104]
- 归属：yotr_fightring

### EndMinigame  [256–311]
- 归属：yotr_fightring

### FlagCheating  [203–205]
- 归属：yotr_fightring

### GetStatus  [879–882]
- 归属：yotr_fightring_bell

### GoToDeactivateMinigame  [208–232]
- 归属：yotr_fightring

### IsArenaClearForMinigame  [111–115]
- 归属：yotr_fightring, yotr_fightring_bell

### IsCompetitorCompeting  [199–201]
- 归属：yotr_fightring

### OnActivateMinigame  [379–386]
- 归属：yotr_fightring

### OnArenaNotClearMessage  [314–321]
- 归属：yotr_fightring

### OnBellActivated  [890–917]
- 归属：yotr_fightring_bell

### OnCameraFocusDirty  [89–95]
- 归属：yotr_fightring

### OnDeactivateMinigame  [388–407]
- 归属：yotr_fightring

### OnFighterArrived  [452–469]
- 归属：yotr_fightring

### OnMusicPlayingDirty  [502–517]
- 归属：yotr_fightring

### OnRingActivated  [410–434]
- 归属：yotr_fightring

### OnRingPlaced  [564–570]
- 归属：yotr_fightring

### OnRingRemoved  [572–590]
- 归属：yotr_fightring

### RegisterWithWorld  [472–482]
- 归属：yotr_fightring

### SetGameToPlaying  [350–355]
- 归属：yotr_fightring

### SetPillowFightActive  [519–524]
- 归属：yotr_fightring

### SetUpAuxiliaryObjects  [546–562]
- 归属：yotr_fightring

### StartMinigame  [357–376]
- 归属：yotr_fightring

### UpdateGameMusic  [496–500]
- 归属：yotr_fightring

### add_fightring_competitor  [65–74]
- 归属：yotr_fightring

### bell_finishgame  [952–958]
- 归属：yotr_fightring_bell

### bell_onplaced  [884–888]
- 归属：yotr_fightring_bell

### bell_play_hit  [919–922]
- 归属：yotr_fightring_bell

### bell_setparentring  [960–965]
- 归属：yotr_fightring_bell

### bellfn  [968–1020]
- 归属：yotr_fightring_bell
- 掉落锚点：1003:

### collect_minigame_fighters  [324–348]
- 归属：yotr_fightring

### create_placer_presentation  [1077–1088]
- 归属：（未归属）

### do_confetti  [239–254]
- 归属：yotr_fightring

### get_fightring_competitors  [84–86]
- 归属：yotr_fightring

### kitfn  [744–793]
- 归属：yotr_fightring_kit

### make_torch_placed  [542–544]
- 归属：yotr_fightring

### make_torch_unplaced  [527–540]
- 归属：yotr_fightring

### on_bell_work_finished  [929–950]
- 归属：yotr_fightring_bell

### on_bell_worked  [924–927]
- 归属：yotr_fightring_bell

### on_kit_deployed  [729–737]
- 归属：yotr_fightring_kit

### onringload  [489–493]
- 归属：yotr_fightring

### onringsave  [485–487]
- 归属：yotr_fightring

### push_cheating  [592–594]
- 归属：yotr_fightring

### push_endofpillowfight  [235–237]
- 归属：yotr_fightring

### remove_fightring_competitor  [76–82]
- 归属：yotr_fightring

### ring_placer_postinit  [1095–1103]
- 归属：（未归属）

### ring_placer_testfn  [1090–1093]
- 归属：（未归属）

### ringfn  [596–715]
- 归属：yotr_fightring

### set_bell_position  [54–62]
- 归属：yotr_fightring

### test_for_reactions  [165–174]
- 归属：yotr_fightring

### test_for_ringout  [118–159]
- 归属：yotr_fightring

### torch_disable_eventpush  [191–196]
- 归属：yotr_fightring

### torch_enable_eventpush  [177–182]
- 归属：yotr_fightring

### torch_gosmall  [820–832]
- 归属：yotr_fightring_torch

### torch_gosmall_eventpush  [184–189]
- 归属：yotr_fightring

### torch_onplaced  [804–807]
- 归属：yotr_fightring_torch

### torch_turnoff  [834–839]
- 归属：yotr_fightring_torch

### torch_turnon  [809–818]
- 归属：yotr_fightring_torch

### torchfn  [841–870]
- 归属：yotr_fightring_torch

### try_react  [161–163]
- 归属：yotr_fightring

