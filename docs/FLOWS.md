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