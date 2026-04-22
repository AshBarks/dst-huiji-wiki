use crate::mapping::{
    FieldMapping, FieldMappingRule, FieldSchema, FieldType, MergeStrategy, Schema, WikiMapper,
};
use crate::models::Recipe;

pub struct RecipeMapper;

impl WikiMapper for Recipe {
    fn schema() -> Schema {
        Schema::new()
            .add_field(
                FieldSchema::new("recipe_name", FieldType::String)
                    .with_title("recipe_name", "配方名称")
                    .required(),
            )
            .add_field(
                FieldSchema::new("ingredient1", FieldType::String)
                    .with_title("ingredient1", "材料1"),
            )
            .add_field(
                FieldSchema::new("amount1", FieldType::Number).with_title("amount1", "材料1数量"),
            )
            .add_field(
                FieldSchema::new("ingredient2", FieldType::String)
                    .with_title("ingredient2", "材料2"),
            )
            .add_field(
                FieldSchema::new("amount2", FieldType::Number).with_title("amount2", "材料2数量"),
            )
            .add_field(
                FieldSchema::new("ingredient3", FieldType::String)
                    .with_title("ingredient3", "材料3"),
            )
            .add_field(
                FieldSchema::new("amount3", FieldType::Number).with_title("amount3", "材料3数量"),
            )
            .add_field(
                FieldSchema::new("ingredient4", FieldType::String)
                    .with_title("ingredient4", "材料4"),
            )
            .add_field(
                FieldSchema::new("amount4", FieldType::Number).with_title("amount4", "材料4数量"),
            )
            .add_field(
                FieldSchema::new("ingredient5", FieldType::String)
                    .with_title("ingredient5", "材料5"),
            )
            .add_field(
                FieldSchema::new("amount5", FieldType::Number).with_title("amount5", "材料5数量"),
            )
            .add_field(
                FieldSchema::new("ingredient6", FieldType::String)
                    .with_title("ingredient6", "材料6"),
            )
            .add_field(
                FieldSchema::new("amount6", FieldType::Number).with_title("amount6", "材料6数量"),
            )
            .add_field(FieldSchema::new("product", FieldType::String).with_title("product", "产物"))
            .add_field(
                FieldSchema::new("numtogive", FieldType::Number)
                    .with_title("numtogive", "产物数量"),
            )
            .add_field(
                FieldSchema::new("override_numtogive_fn", FieldType::Boolean)
                    .with_title("override_numtogive_fn", "产物数量函数"),
            )
            .add_field(
                FieldSchema::new("tech", FieldType::String)
                    .with_title("tech", "科技")
                    .required(),
            )
            .add_field(
                FieldSchema::new("hint_msg", FieldType::String).with_title("hint_msg", "提示信息"),
            )
            .add_field(
                FieldSchema::new("description", FieldType::String)
                    .with_title("description", "描述"),
            )
            .add_field(
                FieldSchema::new("nounlock", FieldType::Boolean).with_title("nounlock", "不可解锁"),
            )
            .add_field(
                FieldSchema::new("no_deconstruction", FieldType::Boolean)
                    .with_title("no_deconstruction", "不可拆解"),
            )
            .add_field(
                FieldSchema::new("unlocks_from_skin", FieldType::Boolean)
                    .with_title("unlocks_from_skin", "皮肤锁定"),
            )
            .add_field(
                FieldSchema::new("station_tag", FieldType::String)
                    .with_title("station_tag", "制作站标签"),
            )
            .add_field(
                FieldSchema::new("builder_tag", FieldType::String)
                    .with_title("builder_tag", "制作者标签"),
            )
            .add_field(
                FieldSchema::new("builder_skill", FieldType::String)
                    .with_title("builder_skill", "制作者技能"),
            )
            .add_field(FieldSchema::new("desc", FieldType::String).with_title("desc", "制作描述"))
    }

    fn mapping_rules() -> Vec<FieldMappingRule<Self>> {
        vec![
            FieldMappingRule {
                target_field: "recipe_name".to_string(),
                mapping: FieldMapping::Direct {
                    source_field: "name".to_string(),
                },
                merge_strategy: MergeStrategy::Overwrite,
            },
            FieldMappingRule {
                target_field: "ingredient1".to_string(),
                mapping: FieldMapping::Computed {
                    compute: |recipe| {
                        recipe
                            .ingredients
                            .first()
                            .map(|ing| serde_json::Value::String(ing.item.clone()))
                            .unwrap_or(serde_json::Value::Null)
                    },
                },
                merge_strategy: MergeStrategy::Overwrite,
            },
            FieldMappingRule {
                target_field: "amount1".to_string(),
                mapping: FieldMapping::Computed {
                    compute: |recipe| {
                        recipe
                            .ingredients
                            .first()
                            .map(|ing| {
                                serde_json::Value::Number(serde_json::Number::from(ing.amount))
                            })
                            .unwrap_or(serde_json::Value::Null)
                    },
                },
                merge_strategy: MergeStrategy::Overwrite,
            },
            FieldMappingRule {
                target_field: "ingredient2".to_string(),
                mapping: FieldMapping::Computed {
                    compute: |recipe| {
                        recipe
                            .ingredients
                            .get(1)
                            .map(|ing| serde_json::Value::String(ing.item.clone()))
                            .unwrap_or(serde_json::Value::Null)
                    },
                },
                merge_strategy: MergeStrategy::Overwrite,
            },
            FieldMappingRule {
                target_field: "amount2".to_string(),
                mapping: FieldMapping::Computed {
                    compute: |recipe| {
                        recipe
                            .ingredients
                            .get(1)
                            .map(|ing| {
                                serde_json::Value::Number(serde_json::Number::from(ing.amount))
                            })
                            .unwrap_or(serde_json::Value::Null)
                    },
                },
                merge_strategy: MergeStrategy::Overwrite,
            },
            FieldMappingRule {
                target_field: "ingredient3".to_string(),
                mapping: FieldMapping::Computed {
                    compute: |recipe| {
                        recipe
                            .ingredients
                            .get(2)
                            .map(|ing| serde_json::Value::String(ing.item.clone()))
                            .unwrap_or(serde_json::Value::Null)
                    },
                },
                merge_strategy: MergeStrategy::Overwrite,
            },
            FieldMappingRule {
                target_field: "amount3".to_string(),
                mapping: FieldMapping::Computed {
                    compute: |recipe| {
                        recipe
                            .ingredients
                            .get(2)
                            .map(|ing| {
                                serde_json::Value::Number(serde_json::Number::from(ing.amount))
                            })
                            .unwrap_or(serde_json::Value::Null)
                    },
                },
                merge_strategy: MergeStrategy::Overwrite,
            },
            FieldMappingRule {
                target_field: "ingredient4".to_string(),
                mapping: FieldMapping::Computed {
                    compute: |recipe| {
                        recipe
                            .ingredients
                            .get(3)
                            .map(|ing| serde_json::Value::String(ing.item.clone()))
                            .unwrap_or(serde_json::Value::Null)
                    },
                },
                merge_strategy: MergeStrategy::Overwrite,
            },
            FieldMappingRule {
                target_field: "amount4".to_string(),
                mapping: FieldMapping::Computed {
                    compute: |recipe| {
                        recipe
                            .ingredients
                            .get(3)
                            .map(|ing| {
                                serde_json::Value::Number(serde_json::Number::from(ing.amount))
                            })
                            .unwrap_or(serde_json::Value::Null)
                    },
                },
                merge_strategy: MergeStrategy::Overwrite,
            },
            FieldMappingRule {
                target_field: "ingredient5".to_string(),
                mapping: FieldMapping::Computed {
                    compute: |recipe| {
                        recipe
                            .ingredients
                            .get(4)
                            .map(|ing| serde_json::Value::String(ing.item.clone()))
                            .unwrap_or(serde_json::Value::Null)
                    },
                },
                merge_strategy: MergeStrategy::Overwrite,
            },
            FieldMappingRule {
                target_field: "amount5".to_string(),
                mapping: FieldMapping::Computed {
                    compute: |recipe| {
                        recipe
                            .ingredients
                            .get(4)
                            .map(|ing| {
                                serde_json::Value::Number(serde_json::Number::from(ing.amount))
                            })
                            .unwrap_or(serde_json::Value::Null)
                    },
                },
                merge_strategy: MergeStrategy::Overwrite,
            },
            FieldMappingRule {
                target_field: "ingredient6".to_string(),
                mapping: FieldMapping::Computed {
                    compute: |recipe| {
                        recipe
                            .ingredients
                            .get(5)
                            .map(|ing| serde_json::Value::String(ing.item.clone()))
                            .unwrap_or(serde_json::Value::Null)
                    },
                },
                merge_strategy: MergeStrategy::Overwrite,
            },
            FieldMappingRule {
                target_field: "amount6".to_string(),
                mapping: FieldMapping::Computed {
                    compute: |recipe| {
                        recipe
                            .ingredients
                            .get(5)
                            .map(|ing| {
                                serde_json::Value::Number(serde_json::Number::from(ing.amount))
                            })
                            .unwrap_or(serde_json::Value::Null)
                    },
                },
                merge_strategy: MergeStrategy::Overwrite,
            },
            FieldMappingRule {
                target_field: "product".to_string(),
                mapping: FieldMapping::Computed {
                    compute: |recipe| {
                        recipe
                            .options
                            .product
                            .as_ref()
                            .map(|s| serde_json::Value::String(s.clone()))
                            .unwrap_or_else(|| serde_json::Value::String(recipe.name.clone()))
                    },
                },
                merge_strategy: MergeStrategy::Overwrite,
            },
            FieldMappingRule {
                target_field: "numtogive".to_string(),
                mapping: FieldMapping::Computed {
                    compute: |recipe| {
                        let n = recipe.options.numtogive.unwrap_or(1);
                        serde_json::Value::Number(serde_json::Number::from(n))
                    },
                },
                merge_strategy: MergeStrategy::Overwrite,
            },
            FieldMappingRule {
                target_field: "override_numtogive_fn".to_string(),
                mapping: FieldMapping::Computed {
                    compute: |recipe| {
                        recipe
                            .options
                            .override_numtogive_fn
                            .map(serde_json::Value::Bool)
                            .unwrap_or(serde_json::Value::Null)
                    },
                },
                merge_strategy: MergeStrategy::Overwrite,
            },
            FieldMappingRule {
                target_field: "tech".to_string(),
                mapping: FieldMapping::Direct {
                    source_field: "tech".to_string(),
                },
                merge_strategy: MergeStrategy::Overwrite,
            },
            FieldMappingRule {
                target_field: "hint_msg".to_string(),
                mapping: FieldMapping::Computed {
                    compute: |recipe| {
                        recipe
                            .options
                            .hint_msg
                            .as_ref()
                            .map(|s| serde_json::Value::String(s.clone()))
                            .unwrap_or(serde_json::Value::Null)
                    },
                },
                merge_strategy: MergeStrategy::Overwrite,
            },
            FieldMappingRule {
                target_field: "description".to_string(),
                mapping: FieldMapping::Computed {
                    compute: |recipe| {
                        recipe
                            .options
                            .description
                            .as_ref()
                            .map(|s| serde_json::Value::String(s.clone()))
                            .unwrap_or(serde_json::Value::Null)
                    },
                },
                merge_strategy: MergeStrategy::Overwrite,
            },
            FieldMappingRule {
                target_field: "nounlock".to_string(),
                mapping: FieldMapping::Computed {
                    compute: |recipe| {
                        recipe
                            .options
                            .nounlock
                            .map(serde_json::Value::Bool)
                            .unwrap_or(serde_json::Value::Null)
                    },
                },
                merge_strategy: MergeStrategy::Overwrite,
            },
            FieldMappingRule {
                target_field: "no_deconstruction".to_string(),
                mapping: FieldMapping::Computed {
                    compute: |recipe| {
                        recipe
                            .options
                            .no_deconstruction
                            .map(serde_json::Value::Bool)
                            .unwrap_or(serde_json::Value::Null)
                    },
                },
                merge_strategy: MergeStrategy::Overwrite,
            },
            FieldMappingRule {
                target_field: "unlocks_from_skin".to_string(),
                mapping: FieldMapping::Computed {
                    compute: |recipe| {
                        recipe
                            .options
                            .unlocks_from_skin
                            .map(serde_json::Value::Bool)
                            .unwrap_or(serde_json::Value::Null)
                    },
                },
                merge_strategy: MergeStrategy::Overwrite,
            },
            FieldMappingRule {
                target_field: "station_tag".to_string(),
                mapping: FieldMapping::Computed {
                    compute: |recipe| {
                        recipe
                            .options
                            .station_tag
                            .as_ref()
                            .map(|s| serde_json::Value::String(s.clone()))
                            .unwrap_or(serde_json::Value::Null)
                    },
                },
                merge_strategy: MergeStrategy::Overwrite,
            },
            FieldMappingRule {
                target_field: "builder_tag".to_string(),
                mapping: FieldMapping::Computed {
                    compute: |recipe| {
                        recipe
                            .options
                            .builder_tag
                            .as_ref()
                            .map(|s| serde_json::Value::String(s.clone()))
                            .unwrap_or(serde_json::Value::Null)
                    },
                },
                merge_strategy: MergeStrategy::Overwrite,
            },
            FieldMappingRule {
                target_field: "builder_skill".to_string(),
                mapping: FieldMapping::Computed {
                    compute: |recipe| {
                        recipe
                            .options
                            .builder_skill
                            .as_ref()
                            .map(|s| serde_json::Value::String(s.clone()))
                            .unwrap_or(serde_json::Value::Null)
                    },
                },
                merge_strategy: MergeStrategy::Overwrite,
            },
            FieldMappingRule {
                target_field: "desc".to_string(),
                mapping: FieldMapping::Computed {
                    compute: |_recipe| serde_json::Value::Null,
                },
                merge_strategy: MergeStrategy::Custom(|new_val, historical_val| {
                    if let serde_json::Value::String(s) = historical_val {
                        if !s.is_empty() {
                            return historical_val.clone();
                        }
                    }
                    new_val.clone()
                }),
            },
        ]
    }

    fn key_field() -> &'static str {
        "recipe_name"
    }

    fn get_field_value(&self, field_name: &str) -> Option<serde_json::Value> {
        match field_name {
            "name" => Some(serde_json::Value::String(self.name.clone())),
            "tech" => Some(serde_json::Value::String(self.tech.clone())),
            "source_file" => self
                .source_file
                .as_ref()
                .map(|s| serde_json::Value::String(s.clone())),
            "source_line" => self
                .source_line
                .map(|n| serde_json::Value::Number(n.into())),
            _ => None,
        }
    }
}
