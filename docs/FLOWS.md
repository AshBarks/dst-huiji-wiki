# 命令流程

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