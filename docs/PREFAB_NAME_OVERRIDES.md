# 预制体名称覆盖

本功能用于统计获取预制体名称覆盖列表。
对于一个lua文件，其中的Prefab("prefab_name", fn, ..)定义了一个预制体，会将"prefab_name"设置为该Prefab的名称。
而fn是定义该预制体的函数。其会返回一个表，假如说是`inst`，`inst`中包含该预制体的所有属性。如果该表在生成过程中调用了`inst:SetPrefabNameOverride("override_name")`，则会将`override_name`设置为该预制体的覆盖名称。

以`examples/prefabs/altar_prototyper.lua`和`examples/prefabs/bundle.lua`为例，试通过语法树分析的方式解析出`prefab_name`到`override_name`的映射关系。
