# `prefabs/pocketwatch.lua`

- 扫描角色：prefabs/pocketwatch.lua
- 归属变体（7 个）：pocketwatch_heal, pocketwatch_recall, pocketwatch_recall_marker, pocketwatch_revive, pocketwatch_revive_reviver, pocketwatch_warp, pocketwatch_warp_marker
## 关联

### 组件
- `components/inventoryitem.lua`：pocketwatch_revive_reviver（Direct；line 101）
- `components/trader.lua`：pocketwatch_recall（Direct；line 297）

### 预制体依赖
- `pocketwatch_cast_fx`：pocketwatch_heal, pocketwatch_recall, pocketwatch_revive, pocketwatch_warp（Direct；line 423,424,426,428）
- `pocketwatch_cast_fx_mount`：pocketwatch_heal, pocketwatch_recall, pocketwatch_revive, pocketwatch_warp（Direct；line 423,424,426,428）
- `pocketwatch_ground_fx`：pocketwatch_heal, pocketwatch_recall, pocketwatch_revive, pocketwatch_warp（Direct；line 423,424,426,428）
- `pocketwatch_heal_fx`：pocketwatch_heal, pocketwatch_recall, pocketwatch_revive, pocketwatch_warp（Direct；line 423,424,426,428）
- `pocketwatch_heal_fx_mount`：pocketwatch_heal, pocketwatch_recall, pocketwatch_revive, pocketwatch_warp（Direct；line 423,424,426,428）
- `pocketwatch_revive_reviver`：pocketwatch_heal, pocketwatch_recall, pocketwatch_revive, pocketwatch_warp（Direct；line 423,424,426,428）
- `pocketwatch_warp_marker`：pocketwatch_heal, pocketwatch_recall, pocketwatch_revive, pocketwatch_warp（Direct；line 423,424,426,428）
- `pocketwatch_warpback_fx`：pocketwatch_heal, pocketwatch_recall, pocketwatch_revive, pocketwatch_warp（Direct；line 423,424,426,428）
- `pocketwatch_warpbackout_fx`：pocketwatch_heal, pocketwatch_recall, pocketwatch_revive, pocketwatch_warp（Direct；line 423,424,426,428）

### 生成引用
- `pocketwatch_revive_reviver`：pocketwatch_revive（Direct；line 122）
- `prefabs/brokentool.lua`：pocketwatch_revive（Direct；line 139）
- `prefabs/pocketwatch_portal.lua`：pocketwatch_recall（Direct；line 261）


## 函数

### DelayedMarkTalker  [219–224]
- 归属：pocketwatch_recall

### Heal_DoCastSpell  [29–41]
- 归属：pocketwatch_heal

### Recall_DoCastSpell  [226–247]
- 归属：pocketwatch_recall

### Recall_GetActionVerb  [281–284]
- 归属：pocketwatch_recall

### Recall_ItemTradeTest  [249–258]
- 归属：pocketwatch_recall

### Recall_OnBuiltFn  [277–279]
- 归属：pocketwatch_recall

### Recall_OnGemGiven  [260–275]
- 归属：pocketwatch_recall

### ReviveOwner  [78–87]
- 归属：pocketwatch_revive_reviver

### Revive_CanTarget  [113–116]
- 归属：pocketwatch_revive

### Revive_DoCastSpell  [118–133]
- 归属：pocketwatch_revive

### Revive_OnHaunt  [135–144]
- 归属：pocketwatch_revive

### Warp_DoCastSpell  [368–382]
- 归属：pocketwatch_warp

### healfn  [45–55]
- 归属：pocketwatch_heal

### recallfn  [288–306]
- 归属：pocketwatch_recall

### recallmarker_RemoveMarker  [177–185]
- 归属：pocketwatch_recall_marker

### recallmarker_ShowMarker  [169–175]
- 归属：pocketwatch_recall_marker

### recallmarkerfn  [187–217]
- 归属：pocketwatch_recall_marker

### revive_onActivateResurrection  [59–71]
- 归属：pocketwatch_revive, pocketwatch_revive_reviver

### revive_reviverfn  [89–109]
- 归属：pocketwatch_revive_reviver

### revive_revivier_onActivateResurrection  [73–76]
- 归属：pocketwatch_revive_reviver

### revivefn  [146–165]
- 归属：pocketwatch_revive

### warp_hidemarker  [386–391]
- 归属：pocketwatch_warp

### warp_showmarker  [394–401]
- 归属：pocketwatch_warp

### warpfn  [403–419]
- 归属：pocketwatch_warp

### warpmarker_HideMarker  [317–323]
- 归属：pocketwatch_warp_marker

### warpmarker_SetMarkerViewer  [309–315]
- 归属：pocketwatch_warp_marker

### warpmarker_ShowMarker  [325–332]
- 归属：pocketwatch_warp_marker

### warpmarkerfn  [334–366]
- 归属：pocketwatch_warp_marker

