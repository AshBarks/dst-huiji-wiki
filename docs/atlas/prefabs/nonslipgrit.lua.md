# `prefabs/nonslipgrit.lua`

- 扫描角色：prefabs/nonslipgrit.lua
- 归属变体（5 个）：nonslipgrit, nonslipgrit_buff, nonslipgrit_buff_fx, nonslipgritboosted, nonslipgritpool
## 关联

### 组件
- `components/debuff.lua`：nonslipgrit_buff（Direct；line 229）
- `components/floater.lua`：nonslipgrit, nonslipgritboosted（HelperExpanded；line 49）
- `components/fueled.lua`：nonslipgrit, nonslipgritboosted（Direct；line 63）
- `components/hauntable.lua`：nonslipgrit, nonslipgritboosted（HelperExpanded；line 61）
- `components/inspectable.lua`：nonslipgrit, nonslipgritboosted（Direct；line 58）
- `components/inventoryitem.lua`：nonslipgrit, nonslipgritboosted（Direct；line 59）
- `components/nonslipgritpool.lua`：nonslipgritpool（Direct；line 136）
- `components/nonslipgritsource.lua`：nonslipgrit, nonslipgritboosted（Direct；line 69）
- `components/timer.lua`：nonslipgrit_buff, nonslipgritpool（Direct；line 139,234）

### 预制体依赖
- `nonslipgrit_buff`：nonslipgrit（Direct；line 377）
- `nonslipgrit_buff_fx`：nonslipgrit_buff（Direct；line 380）
- `nonslipgritpool`：nonslipgritboosted（Direct；line 378）

### 生成引用
- `nonslipgrit_buff_fx`：nonslipgrit_buff（Direct；line 185）
- `nonslipgritpool`：nonslipgrit, nonslipgritboosted（Direct；line 26）


## 函数

### InitEnvelope_buff_fx  [241–271]
- 归属：nonslipgrit_buff_fx

### IntColour  [242–244]
- 归属：nonslipgrit_buff_fx

### IsGritAtPoint  [97–100]
- 归属：nonslipgritpool

### OnAttached_buff  [176–189]
- 归属：nonslipgrit_buff

### OnDelta  [17–20]
- 归属：nonslipgrit, nonslipgritboosted

### OnDelta_Boosted  [24–30]
- 归属：nonslipgrit, nonslipgritboosted

### OnDetached_buff  [191–197]
- 归属：nonslipgrit_buff

### OnExtendedbuff  [199–205]
- 归属：nonslipgrit_buff

### OnInit_pool  [102–109]
- 归属：nonslipgritpool

### OnTimerDone  [87–94]
- 归属：nonslipgritpool

### OnTimerDone_buff  [207–211]
- 归属：nonslipgrit_buff

### buff_fx_emit  [274–302]
- 归属：nonslipgrit_buff_fx

### fn  [79–81]
- 归属：nonslipgrit

### fn_boosted  [83–85]
- 归属：nonslipgritboosted

### fn_buff  [213–238]
- 归属：nonslipgrit_buff

### fn_buff_fx  [304–375]
- 归属：nonslipgrit_buff_fx

### fn_common  [32–77]
- 归属：nonslipgrit, nonslipgritboosted

### fn_pool  [111–148]
- 归属：nonslipgritpool

