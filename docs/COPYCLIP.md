# 解析粘贴功能

将游戏代码里的部分代码，粘贴到wiki的模块指定处。

## 功能说明
以源文件debugcommands.lua和目标模块recipe_builder_tag_lookup.lua(该模块内容目前以文件形式提供，实际使用client获取相应页面的内容)为例，将debugcommands.lua中的代码，粘贴到recipe_builder_tag_lookup.lua中，返回目标模块更新后的字符串。

- 通过字符串匹配和语法树解析识别出RECIPE_BUILDER_TAG_LOOKUP定义开始和结束的位置。
- 将该部分代码，粘贴到recipe_builder_tag_lookup.lua的COPYCLIPSTART和COPYCLIPEND之间。

除示例外，还有若干文件和模块有相应操作，所以将该功能抽象出来。
- 从源代码中根据语法树解析得到的特征定位功能写在parser模块合适的地方里。
- 剩余的功能由你合理规划，写在其他模块合适的地方里。