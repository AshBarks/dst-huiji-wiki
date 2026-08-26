# `prefabs/vault_crawler.lua`

- 扫描角色：prefabs/vault_crawler.lua
- 归属变体（2 个）：vault_crawler, vault_crawler_socket
## 关联

### 组件
- `components/combat.lua`：vault_crawler（Direct；line 231）
- `components/damagetypebonus.lua`：vault_crawler（Direct；line 240）
- `components/damagetyperesist.lua`：vault_crawler（Direct；line 241）
- `components/freezable.lua`：vault_crawler（HelperExpanded；line 249）
- `components/hauntable.lua`：vault_crawler（HelperExpanded；line 250）
- `components/health.lua`：vault_crawler（Direct；line 226）
- `components/inspectable.lua`：vault_crawler（Direct；line 220）
- `components/knownlocations.lua`：vault_crawler（Direct；line 246）
- `components/locomotor.lua`：vault_crawler（Direct；line 222）
- `components/savedrotation.lua`：vault_crawler（Direct；line 247）
- `components/teleportedoverride.lua`：vault_crawler（Direct；line 243）
- `components/updatelooper.lua`：vault_crawler（Direct；line 82）

### 状态图
- `stategraphs/SGvault_crawler.lua`：vault_crawler（Direct；line 133,254）

### 大脑
- `brains/vault_crawlerbrain.lua`：vault_crawler（Direct；line 255）

### 行为
- `brains/vault_crawlerbrain.lua`：ChaseAndAttack, Wander（prefabs/vault_crawler.lua#vault_crawler）


## 函数

### CreateSocketPhysics  [329–349]
- 归属：vault_crawler_socket

### FindSocket  [148–153]
- 归属：vault_crawler

### KeepTargetFn  [34–41]
- 归属：vault_crawler

### OnAttacked  [43–50]
- 归属：vault_crawler

### OnFadeOut  [78–94]
- 归属：vault_crawler

### OnLoadPostPass  [159–166]
- 归属：vault_crawler

### OnRemoveEntity  [140–144]
- 归属：vault_crawler

### OnSave  [155–157]
- 归属：vault_crawler

### OnUpdateFadeOut  [66–76]
- 归属：vault_crawler

### RetargetFn  [10–32]
- 归属：vault_crawler

### SetSocketed  [96–138]
- 归属：vault_crawler

### fn  [169–264]
- 归属：vault_crawler

### socket_CreateFront  [268–285]
- 归属：vault_crawler_socket

### socket_DoOpenSocket  [377–395]
- 归属：vault_crawler_socket

### socket_DoTryOpenSocket  [397–402]
- 归属：vault_crawler_socket

### socket_IsSocketed  [459–461]
- 归属：vault_crawler_socket

### socket_IsSocketedDirty  [416–428]
- 归属：vault_crawler_socket

### socket_OnCollide  [309–327]
- 归属：vault_crawler_socket

### socket_OnEntityWake  [463–481]
- 归属：vault_crawler_socket

### socket_OnLoad  [502–506]
- 归属：vault_crawler_socket

### socket_OnLoadPostPass  [508–515]
- 归属：vault_crawler_socket

### socket_OnOpenAnimOver  [369–373]
- 归属：vault_crawler_socket

### socket_OnRemoveEntity  [483–496]
- 归属：vault_crawler_socket

### socket_OnSave  [498–500]
- 归属：vault_crawler_socket

### socket_OnSocketPhysRadDirty  [351–360]
- 归属：vault_crawler_socket

### socket_OnUpdateCollisionRadius  [287–307]
- 归属：vault_crawler_socket

### socket_SetIsSocketed  [430–457]
- 归属：vault_crawler_socket

### socket_SetSocketRadius  [362–367]
- 归属：vault_crawler_socket

### socket_TryOpenSocket  [404–414]
- 归属：vault_crawler_socket

### socketfn  [517–572]
- 归属：vault_crawler_socket

### teleport_override_fn  [52–57]
- 归属：vault_crawler

