# `prefabs/yotb_stage.lua`

- 扫描角色：prefabs/yotb_stage.lua
- 归属变体（3 个）：yotb_stage, yotb_stage_item, yotb_stage_voice
## 关联

### 组件
- `components/burnable.lua`：yotb_stage_item（HelperExpanded；line 224）
- `components/deployable.lua`：yotb_stage_item（Direct；line 218）
- `components/floater.lua`：yotb_stage_item（HelperExpanded；line 206）
- `components/hauntable.lua`：yotb_stage, yotb_stage_item（Direct；line 115,221）
- `components/inspectable.lua`：yotb_stage, yotb_stage_item（Direct；line 102,214）
- `components/inventoryitem.lua`：yotb_stage_item（Direct；line 216）
- `components/lootdropper.lua`：yotb_stage（Direct；line 104）
- `components/placer.lua`：yotb_stage, yotb_stage_item, yotb_stage_voice（HelperExpanded；line 233）
- `components/propagator.lua`：yotb_stage_item（HelperExpanded；line 225）
- `components/talker.lua`：yotb_stage, yotb_stage_voice（Direct；line 91,145）
- `components/timer.lua`：yotb_stage（Direct；line 113）
- `components/workable.lua`：yotb_stage（Direct；line 105）
- `components/yotb_stager.lua`：yotb_stage（Direct；line 111）

### 状态图
- `stategraphs/SGyotb_stage.lua`：yotb_stage（Direct；line 123）

### 预制体依赖
- `collapse_big`：yotb_stage（Direct；line 230）
- `prefabs/confetti_fx.lua`：yotb_stage（Direct；line 230）
- `prefabs/yotb_stage.lua`：yotb_stage_item（Direct；line 232）
- `yotb_confetti`：yotb_stage（Direct；line 230）
- `yotb_pattern_fragment_1`：yotb_stage（Direct；line 230）
- `yotb_pattern_fragment_2`：yotb_stage（Direct；line 230）
- `yotb_pattern_fragment_3`：yotb_stage（Direct；line 230）
- `yotb_stage_voice`：yotb_stage（Direct；line 230）


## 函数

### fn  [66–130]
- 归属：yotb_stage

### itemfn  [190–228]
- 归属：yotb_stage_item

### onbuilt  [57–59]
- 归属：yotb_stage

### ondeploy  [176–188]
- 归属：yotb_stage_item

### onhammered  [40–47]
- 归属：yotb_stage

### onhit  [49–55]
- 归属：yotb_stage

### onremove  [61–64]
- 归属：yotb_stage

### placer_postinit_fn  [171–174]
- 归属：（未归属）

### voicefn  [132–169]
- 归属：yotb_stage_voice

