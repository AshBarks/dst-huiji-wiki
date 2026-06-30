use crate::mapping::{
    FieldMapping, FieldMappingRule, FieldSchema, FieldType, MergeStrategy, Schema, WikiMapper,
};
use crate::models::Recipe;

/// Declarative macro that generates ingredient and amount field schema entries
/// for slots 1..N, eliminating repetitive hand-written code.
///
/// Usage: `define_ingredient_schema!(schema_builder; 1=>0, 2=>1, 3=>2, 4=>3, 5=>4, 6=>5)`
///
/// Each `n=>idx` pair generates two `FieldSchema` entries:
/// - `ingredient{n}` (String) with title "材料{n}"
/// - `amount{n}` (Number) with title "材料{n}数量"
macro_rules! define_ingredient_schema {
    ($builder:expr; $($n:tt => $idx:tt),* $(,)?) => {
        $(
            $builder = $builder
                .add_field(
                    FieldSchema::new(
                        concat!("ingredient", stringify!($n)),
                        FieldType::String,
                    )
                    .with_title(
                        concat!("ingredient", stringify!($n)),
                        concat!("材料", stringify!($n)),
                    ),
                )
                .add_field(
                    FieldSchema::new(
                        concat!("amount", stringify!($n)),
                        FieldType::Number,
                    )
                    .with_title(
                        concat!("amount", stringify!($n)),
                        concat!("材料", stringify!($n), "数量"),
                    ),
                );
        )*
    };
}

/// Declarative macro that generates ingredient and amount mapping rules
/// for slots 1..N, eliminating repetitive hand-written code.
///
/// Usage: `define_ingredient_rules!(rules_vec; 1=>0, 2=>1, 3=>2, 4=>3, 5=>4, 6=>5)`
///
/// Each `n=>idx` pair generates two `FieldMappingRule::Computed` entries:
/// - `ingredient{n}`: extracts `ingredients[idx].item` as String (or Null)
/// - `amount{n}`: extracts `ingredients[idx].amount` as Number (or Null)
macro_rules! define_ingredient_rules {
    ($rules:expr; $($n:tt => $idx:tt),* $(,)?) => {
        $(
            $rules.push(FieldMappingRule {
                target_field: concat!("ingredient", stringify!($n)).to_string(),
                mapping: FieldMapping::Computed {
                    compute: |recipe: &Recipe| {
                        recipe
                            .ingredients
                            .get($idx)
                            .map(|ing| serde_json::Value::String(ing.item.clone()))
                            .unwrap_or(serde_json::Value::Null)
                    },
                },
                merge_strategy: MergeStrategy::Overwrite,
            });
            $rules.push(FieldMappingRule {
                target_field: concat!("amount", stringify!($n)).to_string(),
                mapping: FieldMapping::Computed {
                    compute: |recipe: &Recipe| {
                        recipe
                            .ingredients
                            .get($idx)
                            .map(|ing| {
                                serde_json::Value::Number(serde_json::Number::from(ing.amount))
                            })
                            .unwrap_or(serde_json::Value::Null)
                    },
                },
                merge_strategy: MergeStrategy::Overwrite,
            });
        )*
    };
}

impl WikiMapper for Recipe {
    fn schema() -> Schema {
        let mut schema = Schema::new().add_field(
            FieldSchema::new("recipe_name", FieldType::String)
                .with_title("recipe_name", "配方名称")
                .required(),
        );

        define_ingredient_schema!(schema; 1=>0, 2=>1, 3=>2, 4=>3, 5=>4, 6=>5);

        schema
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
        let mut rules = vec![FieldMappingRule {
            target_field: "recipe_name".to_string(),
            mapping: FieldMapping::Direct {
                source_field: "name".to_string(),
            },
            merge_strategy: MergeStrategy::Overwrite,
        }];

        define_ingredient_rules!(rules; 1=>0, 2=>1, 3=>2, 4=>3, 5=>4, 6=>5);

        rules.extend([
            FieldMappingRule {
                target_field: "product".to_string(),
                mapping: FieldMapping::Computed {
                    compute: |recipe: &Recipe| {
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
                    compute: |recipe: &Recipe| {
                        let n = recipe.options.numtogive.unwrap_or(1);
                        serde_json::Value::Number(serde_json::Number::from(n))
                    },
                },
                merge_strategy: MergeStrategy::Overwrite,
            },
            FieldMappingRule {
                target_field: "override_numtogive_fn".to_string(),
                mapping: FieldMapping::Computed {
                    compute: |recipe: &Recipe| {
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
                    compute: |recipe: &Recipe| {
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
                    compute: |recipe: &Recipe| {
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
                    compute: |recipe: &Recipe| {
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
                    compute: |recipe: &Recipe| {
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
                    compute: |recipe: &Recipe| {
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
                    compute: |recipe: &Recipe| {
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
                    compute: |recipe: &Recipe| {
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
                    compute: |recipe: &Recipe| {
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
        ]);

        rules
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Ingredient;

    #[test]
    fn test_ingredient_mapping_output() {
        let recipe = Recipe::new(
            "axe".to_string(),
            vec![
                Ingredient::new("twigs".to_string(), 3),
                Ingredient::new("flint".to_string(), 2),
                Ingredient::new("rope".to_string(), 1),
            ],
            "SCIENCE_ONE".to_string(),
        );

        let record = Recipe::to_wiki_record(&recipe);
        let schema = Recipe::schema();

        // Verify ingredient1-3 and amount1-3 have correct values
        let ingredient1_idx = schema.field_index("ingredient1").unwrap();
        let amount1_idx = schema.field_index("amount1").unwrap();
        let ingredient2_idx = schema.field_index("ingredient2").unwrap();
        let amount2_idx = schema.field_index("amount2").unwrap();
        let ingredient3_idx = schema.field_index("ingredient3").unwrap();
        let amount3_idx = schema.field_index("amount3").unwrap();

        assert_eq!(
            record[ingredient1_idx],
            serde_json::Value::String("twigs".to_string())
        );
        assert_eq!(
            record[amount1_idx],
            serde_json::Value::Number(serde_json::Number::from(3))
        );
        assert_eq!(
            record[ingredient2_idx],
            serde_json::Value::String("flint".to_string())
        );
        assert_eq!(
            record[amount2_idx],
            serde_json::Value::Number(serde_json::Number::from(2))
        );
        assert_eq!(
            record[ingredient3_idx],
            serde_json::Value::String("rope".to_string())
        );
        assert_eq!(
            record[amount3_idx],
            serde_json::Value::Number(serde_json::Number::from(1))
        );

        // ingredient4-6 and amount4-6 should be Null (only 3 ingredients)
        for field_name in &[
            "ingredient4",
            "amount4",
            "ingredient5",
            "amount5",
            "ingredient6",
            "amount6",
        ] {
            let idx = schema.field_index(field_name).unwrap();
            assert_eq!(
                record[idx],
                serde_json::Value::Null,
                "expected Null for {field_name}"
            );
        }
    }

    #[test]
    fn test_empty_ingredients() {
        let recipe = Recipe::new("lighter".to_string(), vec![], "TECH.NONE".to_string());

        let record = Recipe::to_wiki_record(&recipe);
        let schema = Recipe::schema();

        // All ingredient1-6 and amount1-6 should be Null
        for n in 1..=6 {
            let ing_idx = schema.field_index(&format!("ingredient{n}")).unwrap();
            let amt_idx = schema.field_index(&format!("amount{n}")).unwrap();
            assert_eq!(
                record[ing_idx],
                serde_json::Value::Null,
                "expected Null for ingredient{n}"
            );
            assert_eq!(
                record[amt_idx],
                serde_json::Value::Null,
                "expected Null for amount{n}"
            );
        }
    }

    #[test]
    fn test_schema_field_count_unchanged() {
        let schema = Recipe::schema();
        assert_eq!(
            schema.fields.len(),
            26,
            "schema should still have 26 fields"
        );
    }

    #[test]
    fn test_mapping_rules_count_unchanged() {
        let rules = Recipe::mapping_rules();
        assert_eq!(rules.len(), 26, "mapping_rules should still have 26 rules");
    }

    #[test]
    fn test_ingredient_schema_types() {
        let schema = Recipe::schema();
        for n in 1..=6 {
            let ing_field = schema
                .fields
                .iter()
                .find(|f| f.name == format!("ingredient{n}"))
                .unwrap();
            assert_eq!(
                ing_field.field_type,
                FieldType::String,
                "ingredient{n} should be String"
            );
            let amt_field = schema
                .fields
                .iter()
                .find(|f| f.name == format!("amount{n}"))
                .unwrap();
            assert_eq!(
                amt_field.field_type,
                FieldType::Number,
                "amount{n} should be Number"
            );
        }
    }

    #[test]
    fn test_ingredient_schema_titles() {
        let schema = Recipe::schema();
        for n in 1..=6 {
            let ing_field = schema
                .fields
                .iter()
                .find(|f| f.name == format!("ingredient{n}"))
                .unwrap();
            let title = ing_field.title.as_ref().unwrap();
            assert_eq!(title.en, format!("ingredient{n}"));
            assert_eq!(title.zh, format!("材料{n}"));

            let amt_field = schema
                .fields
                .iter()
                .find(|f| f.name == format!("amount{n}"))
                .unwrap();
            let title = amt_field.title.as_ref().unwrap();
            assert_eq!(title.en, format!("amount{n}"));
            assert_eq!(title.zh, format!("材料{n}数量"));
        }
    }

    #[test]
    fn test_full_ingredients_round_trip() {
        let recipe = Recipe::new(
            "backpack".to_string(),
            vec![
                Ingredient::new("straw".to_string(), 4),
                Ingredient::new("twigs".to_string(), 2),
                Ingredient::new("rope".to_string(), 1),
                Ingredient::new("silk".to_string(), 3),
                Ingredient::new("goldnugget".to_string(), 5),
                Ingredient::new("cutstone".to_string(), 6),
            ],
            "SCIENCE_ONE".to_string(),
        );

        let record = Recipe::to_wiki_record(&recipe);
        let schema = Recipe::schema();

        let items = ["straw", "twigs", "rope", "silk", "goldnugget", "cutstone"];
        let amounts = [4i32, 2, 1, 3, 5, 6];

        for (i, (item, amount)) in items.iter().zip(amounts.iter()).enumerate() {
            let n = i + 1;
            let ing_idx = schema.field_index(&format!("ingredient{n}")).unwrap();
            let amt_idx = schema.field_index(&format!("amount{n}")).unwrap();

            assert_eq!(
                record[ing_idx],
                serde_json::Value::String(item.to_string()),
                "ingredient{n} mismatch"
            );
            assert_eq!(
                record[amt_idx],
                serde_json::Value::Number(serde_json::Number::from(*amount)),
                "amount{n} mismatch"
            );
        }
    }
}
