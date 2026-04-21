# 模型映射方案

以`PoEntry`为例，说明如何映射到wiki的json结构数据。

这是`PoEntry`的rust结构体：
```rust
pub struct PoEntry {
    pub msgctxt: Option<String>,
    pub msgid: String,
    pub msgstr: String,
    pub comment: Option<String>,
}
```
这是po数据中NAMES分组转化为wiki的json结构数据：

```json
{
    "sources": "Extract data from patch 722900",
    "schema": {
        "fields": [
            {
                "name": "id",
                "type": "string",
                "title": {
                    "en": "id",
                    "zh": ""
                }
            },
            {
                "name": "name_cn",
                "type": "string",
                "title": {
                    "en": "name_cn",
                    "zh": ""
                }
            },
            {
                "name": "name_en",
                "type": "string",
                "title": {
                    "en": "name_en",
                    "zh": ""
                }
            },
            {
                "name": "item_img1",
                "type": "string",
                "title": {
                    "en": "item_img1",
                    "zh": ""
                }
            }
        ]
    },
    "data": [
        [
            "abigail",
            "阿比盖尔",
            "Abigail",
            "Abigail.png"
        ],
        [
            "abigail_flower",
            "阿比盖尔之花",
            "Abigail's Flower",
            "Abigail's Flower DST.png"
        ],
        [
            "abysspillar_minion",
            "追随者",
            "Sequitor",
            "Sequitor.png"
        ],
        [
            "abysspillar_trial",
            "杠杆",
            "Lever",
            "Lever.png"
        ]
    ]
}
```

从rust结构体映射到wiki的json结构数据，需要考虑以下几点：
- 从结构体字段到json字段的映射，可能是直接映射，也可能是通过转换得到的。
- 结构体中有的字段，wiki中没有对应的字段，需要忽略。
- wiki中有的字段，结构体中没有对应的字段，需要额外处理或者有默认处理方式，比如依赖其他字段。
- 需要预定义好json字段的类型，比如字符串、整数、浮点数等。
- 结构体中可能有嵌套结构需要展开扁平化到json字段中。
- 结构体转化为json结构数据时，还要与wiki的json的历史数据比较，对不同字段进行不同的覆盖优先处理，比如直接覆盖，比如与历史数据不同保存历史数据等。

## 具体映射规则

### PoEntry到wiki的Data:ItemTable.tabx的映射规则

所有PoEntry中msgctxt为`STRINGS.NAMES.*`形式的项。

- `msgctxt`映射到`id`字段, 去掉`STRINGS.NAMES.`前缀然后小写, 作为id的值，field_type为`string`。
- `msgstr`映射到`name_cn`字段, field_type为`string`。
- `msgid`映射到`name_en`字段, field_type为`string`。
- PoEntry中没有`item_img1`字段，将msgid添加".png"映射到`item_img1`字段，field_type为`string`，并且有覆盖规则，如果同msgid的值以及存在且不是空字符串，则保留历史数据，否则直接覆盖。
- `comment`忽略。

### Recipe到wiki的Data:DSTRecipes.tabx的映射规则

这是Recipe的wiki中Data:DSTRecipes.tabx的json结构数据：
```json
{
    "license": "CC0-1.0",
    "description": {
        "zh": "饥荒联机版的合成配方列表"
    },
    "sources": "Extract data from patch 722900",
    "schema": {
        "fields": [
            {
                "name": "recipe_name",
                "type": "string",
                "title": {
                    "en": "recipe_name",
                    "zh": "配方名称"
                }
            },
            {
                "name": "ingredient1",
                "type": "string",
                "title": {
                    "en": "ingredient1",
                    "zh": "材料1"
                }
            },
            {
                "name": "amount1",
                "type": "number",
                "title": {
                    "en": "amount1",
                    "zh": "材料1数量"
                }
            },
            {
                "name": "ingredient2",
                "type": "string",
                "title": {
                    "en": "ingredient2",
                    "zh": "材料2"
                }
            },
            {
                "name": "amount2",
                "type": "number",
                "title": {
                    "en": "amount2",
                    "zh": "材料2数量"
                }
            },
            {
                "name": "ingredient3",
                "type": "string",
                "title": {
                    "en": "ingredient3",
                    "zh": "材料3"
                }
            },
            {
                "name": "amount3",
                "type": "number",
                "title": {
                    "en": "amount3",
                    "zh": "材料3数量"
                }
            },
            {
                "name": "ingredient4",
                "type": "string",
                "title": {
                    "en": "ingredient4",
                    "zh": "材料4"
                }
            },
            {
                "name": "amount4",
                "type": "number",
                "title": {
                    "en": "amount4",
                    "zh": "材料4数量"
                }
            },
            {
                "name": "ingredient5",
                "type": "string",
                "title": {
                    "en": "ingredient5",
                    "zh": "材料5"
                }
            },
            {
                "name": "amount5",
                "type": "number",
                "title": {
                    "en": "amount5",
                    "zh": "材料5数量"
                }
            },
            {
                "name": "ingredient6",
                "type": "string",
                "title": {
                    "en": "ingredient6",
                    "zh": "材料6"
                }
            },
            {
                "name": "amount6",
                "type": "number",
                "title": {
                    "en": "amount6",
                    "zh": "材料6数量"
                }
            },
            {
                "name": "product",
                "type": "string",
                "title": {
                    "en": "product",
                    "zh": "产物"
                }
            },
            {
                "name": "numtogive",
                "type": "number",
                "title": {
                    "en": "numtogive",
                    "zh": "产物数量"
                }
            },
            {
                "name": "override_numtogive_fn",
                "type": "boolean",
                "title": {
                    "en": "override_numtogive_fn",
                    "zh": "产物数量函数"
                }
            },
            {
                "name": "tech",
                "type": "string",
                "title": {
                    "en": "tech",
                    "zh": "科技"
                }
            },
            {
                "name": "hint_msg",
                "type": "string",
                "title": {
                    "en": "hint_msg",
                    "zh": "提示信息"
                }
            },
            {
                "name": "description",
                "type": "string",
                "title": {
                    "en": "description",
                    "zh": "描述"
                }
            },
            {
                "name": "nounlock",
                "type": "boolean",
                "title": {
                    "en": "nounlock",
                    "zh": "不可解锁"
                }
            },
            {
                "name": "no_deconstruction",
                "type": "boolean",
                "title": {
                    "en": "no_deconstruction",
                    "zh": "不可拆解"
                }
            },
            {
                "name": "unlocks_from_skin",
                "type": "boolean",
                "title": {
                    "en": "unlocks_from_skin",
                    "zh": "皮肤锁定"
                }
            },
            {
                "name": "station_tag",
                "type": "string",
                "title": {
                    "en": "station_tag",
                    "zh": "制作站标签"
                }
            },
            {
                "name": "builder_tag",
                "type": "string",
                "title": {
                    "en": "builder_tag",
                    "zh": "制作者标签"
                }
            },
            {
                "name": "builder_skill",
                "type": "string",
                "title": {
                    "en": "builder_skill",
                    "zh": "制作者技能"
                }
            },
            {
                "name": "desc",
                "type": "string",
                "title": {
                    "en": "desc",
                    "zh": "制作描述"
                }
            }
        ]
    },
    "data": [
        [
            "lighter",
            "rope",
            1,
            "goldnugget",
            1,
            "petals",
            3,
            null,
            null,
            null,
            null,
            null,
            null,
            "lighter",
            1,
            null,
            "TECH.NONE",
            null,
            null,
            null,
            null,
            null,
            null,
            "pyromaniac",
            null,
            "火焰在雨中彻夜燃烧。"
        ],
        [
            "bernie_inactive",
            "beardhair",
            2,
            "beefalowool",
            2,
            "silk",
            2,
            null,
            null,
            null,
            null,
            null,
            null,
            "bernie_inactive",
            1,
            null,
            "TECH.NONE",
            null,
            null,
            null,
            null,
            null,
            null,
            "pyromaniac",
            null,
            "这个疯狂的世界总有你熟悉的人。"
        ],
        [
            "portablecookpot_item",
            "goldnugget",
            2,
            "charcoal",
            6,
            "twigs",
            6,
            null,
            null,
            null,
            null,
            null,
            null,
            "portablecookpot_item",
            1,
            null,
            "TECH.NONE",
            null,
            null,
            null,
            null,
            null,
            null,
            "masterchef",
            null,
            "随时随地为美食家服务。"
        ]
    ]
}
```

- 结构体name字段映射到`recipe_name`字段, field_type为`string`。
- ingredients字段是一个数组，每个元素是一个数组，包含item_id和num。最多会有6个，映射到`ingredients1`到`ingredients6`字段(field_type为`string`)以及`amount1`到`amount6`字段(field_type为`int`)。如果数组没有足够的元素，则用null填充。
- opstions中的`product`字段映射到`product`字段, field_type为`string`，如果该字段为None，则使用name字段的值。
- options中`numtogive`字段映射到`numtogive`字段, field_type为`int`。
- `tech`字段映射到`tech`字段, field_type为`string`，该字段应该总以`TECH.`开头。
- `hint_msg`字段映射到`hint_msg`字段, field_type为`string`，RecipeOptions中没有定义该字段，这意味着解析时漏了该字段，应当修复相关解析代码。
- option中`description`字段映射到`description`字段, field_type为`string`。
- `nounlock`字段映射到`nounlock`字段, field_type为`boolean`。
- `no_deconstruction`字段映射到`no_deconstruction`字段, field_type为`boolean`，有时该字段在提取时是函数，此时视为`false`。
- `unlocks_from_skin`字段映射到`unlocks_from_skin`字段,RecipeOptions中没有定义该字段，这意味着解析时漏了该字段，应当修复相关解析代码。field_type为`boolean`。
- `station_tag`字段映射到`station_tag`字段, field_type为`string`,RecipeOptions中没有定义该字段，这意味着解析时漏了该字段，应当修复相关解析代码。
- `builder_tag`字段映射到`builder_tag`字段, field_type为`string`,RecipeOptions中没有定义该字段，这意味着解析时漏了该字段，应当修复相关解析代码。
- `builder_skill`字段映射到`builder_skill`字段, field_type为`string`。
- Recipe中没有`desc`字段,应当使用`STRINGS.RECIPE_DESC.`拼接`description`字段的大写作为msgctxt在PoEntry中寻找到对应数据，使用其msgstr作为`desc`字段的值，如果`description`字段为空，则使用`STRINGS.RECIPE_DESC.`拼接`recipe_name`字段的大写作为msgctxt在PoEntry中寻找到对应数据，使用其msgstr作为`desc`字段的值。 field_type为`string`。
