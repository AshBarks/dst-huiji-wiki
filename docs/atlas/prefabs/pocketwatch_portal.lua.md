# `prefabs/pocketwatch_portal.lua`

- 扫描角色：prefabs/pocketwatch_portal.lua
- 归属变体（6 个）：pocketwatch_portal, pocketwatch_portal_entrance, pocketwatch_portal_entrance_overlay, pocketwatch_portal_entrance_underlay, pocketwatch_portal_exit, pocketwatch_portal_exit_fx
## 关联

### 组件
- `components/inspectable.lua`：pocketwatch_portal_entrance, pocketwatch_portal_exit, pocketwatch_portal_exit_fx（Direct；line 280,423,500）
- `components/teleporter.lua`：pocketwatch_portal_entrance, pocketwatch_portal_exit（Direct；line 287,425）
- `components/timer.lua`：pocketwatch_portal_entrance（Direct；line 283）

### 预制体依赖
- `pocketwatch_portal_entrance`：pocketwatch_portal（Direct；line 509）
- `pocketwatch_portal_entrance_overlay`：pocketwatch_portal_entrance（Direct；line 510）
- `pocketwatch_portal_entrance_underlay`：pocketwatch_portal_entrance（Direct；line 510）
- `pocketwatch_portal_exit`：pocketwatch_portal_entrance（Direct；line 510）

### 生成引用
- `pocketwatch_portal_entrance`：pocketwatch_portal（Direct；line 58）
- `pocketwatch_portal_entrance_overlay`：pocketwatch_portal_entrance（Direct；line 297）
- `pocketwatch_portal_entrance_underlay`：pocketwatch_portal_entrance（Direct；line 300）
- `pocketwatch_portal_exit`：pocketwatch_portal_entrance（Direct；line 209）
- `pocketwatch_recall`：pocketwatch_portal（Direct；line 63）


## 函数

### CloseEntrance  [166–188]
- 归属：pocketwatch_portal_entrance

### CloseExit  [157–164]
- 归属：pocketwatch_portal_entrance, pocketwatch_portal_exit

### DelayedMarkTalker  [31–36]
- 归属：pocketwatch_portal

### DoCastSpell  [42–90]
- 归属：pocketwatch_portal

### Exit_DoneTeleportingTalker  [389–391]
- 归属：pocketwatch_portal_exit

### Exit_OnDoneTeleporting  [393–403]
- 归属：pocketwatch_portal_exit

### GetActionVerb  [92–95]
- 归属：pocketwatch_portal

### GetStatus  [232–235]
- 归属：pocketwatch_portal_entrance

### OnActivate  [222–230]
- 归属：pocketwatch_portal_entrance

### OnLightDirty  [150–155]
- 归属：pocketwatch_portal_entrance, pocketwatch_portal_exit_fx

### OnLoadPostPass  [382–387]
- 归属：pocketwatch_portal_exit

### OnSave  [378–380]
- 归属：pocketwatch_portal_exit

### OnTimerDone  [190–203]
- 归属：pocketwatch_portal_entrance

### OnUpdateLight  [130–148]
- 归属：pocketwatch_portal_entrance, pocketwatch_portal_exit_fx

### SpawnExit  [205–220]
- 归属：pocketwatch_portal_entrance

### fn  [111–125]
- 归属：pocketwatch_portal

### noentcheckfn  [38–40]
- 归属：pocketwatch_portal

### onPreBuilt  [97–109]
- 归属：pocketwatch_portal

### overlay_OnEntityReplicated  [306–310]
- 归属：pocketwatch_portal_entrance_overlay

### portal_entrance_fn  [237–304]
- 归属：pocketwatch_portal_entrance

### portal_entrance_overlayfn  [312–341]
- 归属：pocketwatch_portal_entrance_overlay

### portal_entrance_underlayfn  [343–373]
- 归属：pocketwatch_portal_entrance_underlay

### portal_exit_fn  [405–435]
- 归属：pocketwatch_portal_exit

### portal_exit_fx_fn  [450–505]
- 归属：pocketwatch_portal_exit_fx

### portal_fx_close  [437–448]
- 归属：pocketwatch_portal_exit_fx

