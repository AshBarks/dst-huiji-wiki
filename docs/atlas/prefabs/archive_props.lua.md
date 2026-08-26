# `prefabs/archive_props.lua`

- 扫描角色：prefabs/archive_props.lua
- 归属变体（13 个）：archive_ambient_sfx, archive_moon_statue, archive_portal, archive_rune_statue, archive_security_desk, archive_security_pulse, archive_security_pulse_sfx, archive_security_waypoint, archive_switch, archive_switch_base, archive_switch_pad, rubble1, rubble2
## 关联

### 组件
- `components/childspawner.lua`：archive_security_desk（Direct；line 361）
- `components/hauntable.lua`：archive_moon_statue（HelperExpanded；line 160）
- `components/inspectable.lua`：archive_moon_statue, archive_portal, archive_rune_statue, archive_security_desk, archive_switch（Direct；line 149,254,358,804,995）
- `components/locomotor.lua`：archive_security_pulse（Direct；line 486）
- `components/lootdropper.lua`：archive_moon_statue（Direct；line 157）
- `components/pickable.lua`：archive_switch（Direct；line 807）
- `components/playerprox.lua`：archive_security_desk（Direct；line 379）
- `components/pointofinterest.lua`：archive_switch_base（Direct；line 884）
- `components/trader.lua`：archive_switch（Direct；line 811）
- `components/updatelooper.lua`：archive_security_desk, archive_security_pulse_sfx（Direct；line 376,542）
- `components/workable.lua`：archive_moon_statue（Direct；line 151）

### 大脑
- `brains/archive_securitypulsebrain.lua`：archive_security_pulse（Direct；line 499）

### 预制体依赖
- `archive_dispencer_sfx`：archive_switch（Direct；line 1053）
- `archive_security_pulse`：archive_security_desk（Direct；line 1049）
- `archive_security_pulse_sfx`：archive_security_pulse（Direct；line 1050）
- `archive_security_waypoint`：archive_security_desk（Direct；line 1049）
- `archive_switch_base`：archive_switch（Direct；line 1053）
- `archive_switch_pad`：archive_switch（Direct；line 1053）
- `grotto_war_sfx`：archive_switch（Direct；line 1053）

### 行为
- `brains/archive_securitypulsebrain.lua`：Follow, StandStill（prefabs/archive_props.lua#archive_security_pulse）


## 函数

### CreateDropShadow  [927–951]
- 归属：archive_portal

### DestroyGem  [741–745]
- 归属：archive_switch

### FindSecurityPulseTarget  [421–436]
- 归属：archive_security_pulse

### ItemTradeTestSwitch  [550–557]
- 归属：archive_switch

### OnGemGiven  [684–709]
- 归属：archive_switch

### OnGemTaken  [711–731]
- 归属：archive_switch

### OnLoadPostPassSwitch  [753–769]
- 归属：archive_switch

### OnLocomote  [438–444]
- 归属：archive_security_pulse

### OnSaveSwitch  [747–751]
- 归属：archive_switch

### OnUpdateDesk  [271–306]
- 归属：archive_security_desk

### OnUpdatePulseSFX  [504–515]
- 归属：archive_security_pulse_sfx

### OnWorkFinished  [90–97]
- 归属：archive_moon_statue

### SetSfxPosition  [446–450]
- 归属：archive_security_pulse

### ShatterGem  [733–739]
- 归属：archive_switch

### ShowWorkState  [79–88]
- 归属：archive_moon_statue

### ambientfn  [1003–1027]
- 归属：archive_ambient_sfx

### canspawn  [264–269]
- 归属：archive_security_desk

### checkforgems  [659–682]
- 归属：archive_switch

### findwaypoints  [569–579]
- 归属：archive_switch

### getStatusPower  [308–312]
- 归属：archive_security_desk

### getstatus  [180–183]
- 归属：archive_rune_statue

### getstatusSwitch  [771–773]
- 归属：archive_switch

### getstatusportal  [953–957]
- 归属：archive_portal

### onloadRune  [197–212]
- 归属：archive_rune_statue

### onloadpostpass  [107–113]
- 归属：archive_moon_statue

### onsave  [103–105]
- 归属：archive_moon_statue

### onsaveRune  [192–195]
- 归属：archive_rune_statue

### portalfn  [959–1001]
- 归属：archive_portal

### rune_AdvanceStory  [173–178]
- 归属：archive_rune_statue

### rune_getdescription  [185–190]
- 归属：archive_rune_statue

### runefn  [214–262]
- 归属：archive_rune_statue

### securityfn  [314–390]
- 归属：archive_security_desk

### securitypulse_sfxfn  [517–548]
- 归属：archive_security_pulse_sfx

### securitypulsefn  [452–502]
- 归属：archive_security_pulse

### securitywaypointfn  [392–408]
- 归属：archive_security_waypoint

### setminimapiconstatue  [99–101]
- 归属：archive_moon_statue

### spawnsounderobj  [581–586]
- 归属：archive_switch

### startpowersound  [602–655]
- 归属：archive_switch

### startshadowwar  [559–566]
- 归属：archive_switch

### statuefn  [115–168]
- 归属：archive_moon_statue

### switchbasefn  [868–925]
- 归属：archive_switch_base

### switchfn  [775–838]
- 归属：archive_switch

### switchpadfn  [841–865]
- 归属：archive_switch_pad

### testbetweenpoints  [588–599]
- 归属：archive_switch

### worldgenitemfn  [1029–1044]
- 归属：rubble1, rubble2

