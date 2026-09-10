# 命令流程

## scripts-sync 同步流程（纯本地）
从环境变量中读取`DST__ROOT`，验证DST目录是否存在。
- 读取`<DST>/version.txt`与状态文件`dst_version.txt`对比版本；一致则跳过（`--force` 可强制）
- 解压`data/databundles/scripts.zip`到`incoming_<时间戳>`暂存目录（原子性：成功前不动现存树）
- 将现存`scripts/`树重命名为`scripts_<yyyymmddhhmm>`快照（`DstContext::list_snapshots` 消费此命名）
- 暂存树移入原位（失败回滚归档），写入新版本到状态文件

## images-sync 图片管线流程（纯本地）
从环境变量中读取`DST__ROOT`与`KTOOLS__OUT_DIR`（缺省`output/ktools`），验证两源存在。
- **两源盘点**：直读`data/databundles/images.zip`条目（不解压）+ 递归遍历`data/images/`散装目录，按 base 名合并 xml↔tex（同名冲突以 loose 为准，遗留 png 仅计数）
- **解压**`images.zip`到`current/unzipped/`（每轮清空重建）
- **解码+切割**：内置 KTEX 解码器（ktex-rs，DXT1/3/5/RGB，翻转+反预乘）解码 mipmap 0，按 atlas XML 的 UV 坐标（v 轴翻转、inclusive 边界）裁出独立 sprite 到`current/split/<atlas>/`
- **独立图**：无同名 xml 的 tex 解码为`current/decoded/<base>.png`
- **差异历史**：最终产物（切片+独立图）按内容 hash 入`history/objects/`（CAS，跨版本去重）；每 build 一份`history/manifests/<build>.json`记录全量清单 + 相对上一完整版本的 added/removed/changed + 输入 hash
- **增量**：输入 hash 与 manifest 一致且产物在盘则跳过；解码器版本变更等效`--force`全量重处理；失败文件清理旧产物，partial 运行不作为下个 diff 基线
- **对账**：`current/` 只保留本次保证的产物（移除陈旧文件与空目录）

## skilltree-export 技能树本地导出流程（纯本地）
从环境变量中读取`DST__ROOT`，验证`data/databundles/scripts/prefabs/`（或快照目录）存在。
- 扫描`skilltree_<char>.lua`获得全部角色（支持`--character`子串过滤）
- 解析`languages/chinese_s.po`中的`STRINGS.SKILLTREE.*`中文标题/描述
- 解析每个角色的技能树，生成包含`character`/`groups`/`nodes`的 JSON
- 写入`--output`目录（默认`output/skilltree`），每个角色一个`<角色>.json`
- 纯本地操作，不访问维基，不需要`HUIJI__*`凭据

## skilltree-wiki 技能树子页面维护流程
从环境变量中读取`DST__ROOT`，验证DST目录存在。
- 扫描`skilltree_<char>.lua`获得全部角色（支持`--character`子串过滤）
- 解析`languages/chinese_s.po`中的技能树中文标题/描述
- 从维基获取`模块:Skilltree/<Char>`现有页面，保留`metainfo`与旧`defs`中的`icon_url`
- 组装子页面`defs` JSON并用`return [[ ... ]]`包裹为页面内容；`defs` 额外提取
  `decorations`（薇诺娜货架等多背景图，含位置/尺寸/缩放）
- `metainfo.render` 写入游戏部件几何（背景矩形/节点偏移/XP 位置/背景 tint），
  渲染器据此布局；`metainfo.imgs` 与角色所需图片清单合并，缺失键留空并
  在日志/`--report-json`里报告（手工上传后回填 URL，下次运行自动保留）
- `--output`可同时把每个子页面内容写到本地目录（`<Char>.lua`），并写出
  更新版渲染器`Skilltree.js`（复制到维基`零件:Skilltree.js`）
- 按`--yes`/`--dry-run`/交互确认决定是否写入维基

## ItemTable维护流程
从环境变量中读取`DST__ROOT`，验证DST目录是否存在。
- 从DST目录中读取`data/databundles/scripts.zip`文件。
- 解压该文件读取scripts/languages/chinese_s.po文件
- 使用client从`Data:ItemTable.tabx`页面获取历史json数据
- 使用PoEntry解析该文件，转化为wiki的json数据
- 对比数据，输出差异


## DSTRecipes维护流程
从环境变量中读取`DST__ROOT`，验证DST目录是否存在。
- 从DST目录中读取`data/databundles/scripts.zip`文件。
- 解压该文件读取scripts/recipes.lua文件
- 使用RecipeParser解析该文件，转化为wiki的json数据
- 使用client从`Data:DSTRecipes.tabx`页面获取历史json数据
- 对比数据，输出差异

## 模块自动复制粘贴流程
从环境变量中读取`DST__ROOT`，验证DST目录是否存在。
- 从DST目录中读取`data/databundles/scripts.zip`文件。
- 解压该文件读取scripts/recipes.lua文件

### RecipeBuilderTagLookup
- 用client获取`模块:Constants/RecipeBuilderTagLookup`页面的内容
- 从`scripts.zip`中读取scripts/debugcommands.lua文件中复制RECIPE_BUILDER_TAG_LOOKUP定义
- 粘贴到获取的页面内容的COPYCLIPSTART和COPYCLIPEND之间
- 输出粘贴后的字符串

### Tech
- 用client获取`模块:Constants/Tech`页面的内容
- 从`scripts.zip`中读取scripts/constants.lua文件中复制TECH定义
- 粘贴到获取的页面内容的COPYCLIPSTART和COPYCLIPEND之间
- 输出粘贴后的字符串

### CraftingFilters
- 用client获取`模块:Constants/CraftingFilters`页面的内容
- 从`scripts.zip`中读取scripts/recipes_filter.lua文件中复制CRAFTING_FILTERS.CHARACTER.recipes定义到CRAFTING_FILTERS.DECOR.recipes定义
- 粘贴到获取的页面内容的COPYCLIPSTART和COPYCLIPEND之间
- 输出粘贴后的字符串

### CraftingNames
- 用client获取`模块:Constants/CraftingNames`页面的内容
- PoEntry解析chinese_s.po文件后，过滤msgctxt以`STRINGS.UI.CRAFTING_STATION_FILTERS.`开头和`STRINGS.UI.CRAFTING_FILTERS.`开头的项，整理成examples/crafting_names.json示例的json形式
- 粘贴到获取的页面内容的`[[`和`]]`之间
- 输出粘贴后的字符串