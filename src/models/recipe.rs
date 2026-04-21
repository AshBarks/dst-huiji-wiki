use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ingredient {
    pub item: String,
    pub amount: i32,
    pub atlas: Option<String>,
    pub image: Option<String>,
}

impl Ingredient {
    pub fn new(item: String, amount: i32) -> Self {
        Self {
            item,
            amount,
            atlas: None,
            image: None,
        }
    }

    pub fn with_atlas(mut self, atlas: String) -> Self {
        self.atlas = Some(atlas);
        self
    }

    pub fn with_image(mut self, image: String) -> Self {
        self.image = Some(image);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RecipeOptions {
    pub builder_tag: Option<String>,
    pub builder_skill: Option<String>,
    pub numtogive: Option<i32>,
    pub product: Option<String>,
    pub placer: Option<String>,
    pub image: Option<String>,
    pub nounlock: Option<bool>,
    pub no_deconstruction: Option<bool>,
    pub min_spacing: Option<f32>,
    pub testfn: Option<String>,
    pub action_str: Option<String>,
    pub filter_text: Option<String>,
    pub sg_state: Option<String>,
    pub description: Option<String>,
    pub override_numtogive_fn: Option<bool>,
    pub is_crafting_station: Option<bool>,
    pub icon_atlas: Option<String>,
    pub icon_image: Option<String>,
    pub hint_msg: Option<String>,
    pub unlocks_from_skin: Option<bool>,
    pub station_tag: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recipe {
    pub name: String,
    pub ingredients: Vec<Ingredient>,
    pub tech: String,
    pub options: RecipeOptions,
    pub source_file: Option<String>,
    pub source_line: Option<usize>,
}

impl Recipe {
    pub fn new(name: String, ingredients: Vec<Ingredient>, tech: String) -> Self {
        Self {
            name,
            ingredients,
            tech,
            options: RecipeOptions::default(),
            source_file: None,
            source_line: None,
        }
    }

    pub fn with_options(mut self, options: RecipeOptions) -> Self {
        self.options = options;
        self
    }

    pub fn with_source(mut self, file: String, line: usize) -> Self {
        self.source_file = Some(file);
        self.source_line = Some(line);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrototyperDef {
    pub name: String,
    pub icon_atlas: Option<String>,
    pub icon_image: Option<String>,
    pub is_crafting_station: Option<bool>,
    pub action_str: Option<String>,
    pub filter_text: Option<String>,
}

impl PrototyperDef {
    pub fn new(name: String) -> Self {
        Self {
            name,
            icon_atlas: None,
            icon_image: None,
            is_crafting_station: None,
            action_str: None,
            filter_text: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RecipeContext {
    pub recipes: Vec<Recipe>,
    pub prototyper_defs: Vec<PrototyperDef>,
    pub tech_constants: std::collections::HashMap<String, String>,
    pub variables: std::collections::HashMap<String, String>,
    pub character_ingredients: std::collections::HashMap<String, String>,
    pub tech_ingredients: std::collections::HashMap<String, String>,
    pub tuning_constants: std::collections::HashMap<String, i32>,
}

impl RecipeContext {
    pub fn new() -> Self {
        let mut ctx = Self {
            recipes: Vec::new(),
            prototyper_defs: Vec::new(),
            tech_constants: std::collections::HashMap::new(),
            variables: std::collections::HashMap::new(),
            character_ingredients: std::collections::HashMap::new(),
            tech_ingredients: std::collections::HashMap::new(),
            tuning_constants: std::collections::HashMap::new(),
        };
        ctx.init_tech_constants();
        ctx.init_ingredient_constants();
        ctx.init_tuning_constants();
        ctx
    }

    fn init_tech_constants(&mut self) {
        let tech_levels = [
            ("TECH.NONE", "NONE"),
            ("TECH.SCIENCE_ONE", "SCIENCE_ONE"),
            ("TECH.SCIENCE_TWO", "SCIENCE_TWO"),
            ("TECH.MAGIC_TWO", "MAGIC_TWO"),
            ("TECH.MAGIC_THREE", "MAGIC_THREE"),
            ("TECH.ANCIENT_TWO", "ANCIENT_TWO"),
            ("TECH.ANCIENT_FOUR", "ANCIENT_FOUR"),
            ("TECH.FOODPROCESSING_ONE", "FOODPROCESSING_ONE"),
            ("TECH.CELESTIAL_ONE", "CELESTIAL_ONE"),
            ("TECH.CELESTIAL_TWO", "CELESTIAL_TWO"),
            ("TECH.CELESTIAL_THREE", "CELESTIAL_THREE"),
            ("TECH.SHADOW_ONE", "SHADOW_ONE"),
            ("TECH.SHADOW_TWO", "SHADOW_TWO"),
            ("TECH.SHADOW_THREE", "SHADOW_THREE"),
            ("TECH.CARNIVAL_HOSTSHOP", "CARNIVAL_HOSTSHOP"),
            ("TECH.CARNIVAL_PRIZESHOP", "CARNIVAL_PRIZESHOP"),
        ];
        for (key, value) in tech_levels {
            self.tech_constants.insert(key.to_string(), value.to_string());
        }
    }

    fn init_ingredient_constants(&mut self) {
        let character_ingredients = [
            ("CHARACTER_INGREDIENT.HEALTH", "decrease_health"),
            ("CHARACTER_INGREDIENT.MAX_HEALTH", "half_health"),
            ("CHARACTER_INGREDIENT.SANITY", "decrease_sanity"),
            ("CHARACTER_INGREDIENT.MAX_SANITY", "half_sanity"),
            ("CHARACTER_INGREDIENT.OLDAGE", "decrease_oldage"),
        ];
        for (key, value) in character_ingredients {
            self.character_ingredients.insert(key.to_string(), value.to_string());
        }

        let tech_ingredients = [
            ("TECH_INGREDIENT.SCULPTING", "sculpting_material"),
        ];
        for (key, value) in tech_ingredients {
            self.tech_ingredients.insert(key.to_string(), value.to_string());
        }
    }

    pub fn resolve_tech(&self, tech_expr: &str) -> String {
        tech_expr.to_string()
    }

    pub fn resolve_ingredient(&self, item_expr: &str) -> Result<String, String> {
        if item_expr.starts_with("CHARACTER_INGREDIENT.") || item_expr.starts_with("TECH_INGREDIENT.") {
            if let Some(resolved) = self.character_ingredients.get(item_expr) {
                return Ok(resolved.clone());
            }
            if let Some(resolved) = self.tech_ingredients.get(item_expr) {
                return Ok(resolved.clone());
            }
            return Err(format!("Unknown ingredient constant: {}", item_expr));
        }
        Ok(item_expr.to_string())
    }

    fn init_tuning_constants(&mut self) {
        let tuning_constants = [
            ("TUNING.EFFIGY_HEALTH_PENALTY", 40),
        ];
        for (key, value) in tuning_constants {
            self.tuning_constants.insert(key.to_string(), value);
        }
    }

    pub fn resolve_tuning(&self, expr: &str) -> Option<i32> {
        self.tuning_constants.get(expr).copied()
    }
}

impl crate::mapping::WikiMapper for Recipe {
    fn schema() -> crate::mapping::Schema {
        use crate::mapping::{FieldSchema, FieldType, Schema};

        Schema::new()
            .add_field(
                FieldSchema::new("recipe_name", FieldType::String)
                    .with_title("recipe_name", "配方名称")
                    .required(),
            )
            .add_field(
                FieldSchema::new("ingredient1", FieldType::String).with_title("ingredient1", "材料1"),
            )
            .add_field(
                FieldSchema::new("amount1", FieldType::Integer).with_title("amount1", "材料1数量"),
            )
            .add_field(
                FieldSchema::new("ingredient2", FieldType::String).with_title("ingredient2", "材料2"),
            )
            .add_field(
                FieldSchema::new("amount2", FieldType::Integer).with_title("amount2", "材料2数量"),
            )
            .add_field(
                FieldSchema::new("ingredient3", FieldType::String).with_title("ingredient3", "材料3"),
            )
            .add_field(
                FieldSchema::new("amount3", FieldType::Integer).with_title("amount3", "材料3数量"),
            )
            .add_field(
                FieldSchema::new("ingredient4", FieldType::String).with_title("ingredient4", "材料4"),
            )
            .add_field(
                FieldSchema::new("amount4", FieldType::Integer).with_title("amount4", "材料4数量"),
            )
            .add_field(
                FieldSchema::new("ingredient5", FieldType::String).with_title("ingredient5", "材料5"),
            )
            .add_field(
                FieldSchema::new("amount5", FieldType::Integer).with_title("amount5", "材料5数量"),
            )
            .add_field(
                FieldSchema::new("ingredient6", FieldType::String).with_title("ingredient6", "材料6"),
            )
            .add_field(
                FieldSchema::new("amount6", FieldType::Integer).with_title("amount6", "材料6数量"),
            )
            .add_field(FieldSchema::new("product", FieldType::String).with_title("product", "产物"))
            .add_field(
                FieldSchema::new("numtogive", FieldType::Integer).with_title("numtogive", "产物数量"),
            )
            .add_field(
                FieldSchema::new("override_numtogive_fn", FieldType::Boolean)
                    .with_title("override_numtogive_fn", "产物数量函数"),
            )
            .add_field(FieldSchema::new("tech", FieldType::String).with_title("tech", "科技").required())
            .add_field(
                FieldSchema::new("hint_msg", FieldType::String).with_title("hint_msg", "提示信息"),
            )
            .add_field(
                FieldSchema::new("description", FieldType::String).with_title("description", "描述"),
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
                FieldSchema::new("station_tag", FieldType::String).with_title("station_tag", "制作站标签"),
            )
            .add_field(
                FieldSchema::new("builder_tag", FieldType::String).with_title("builder_tag", "制作者标签"),
            )
            .add_field(
                FieldSchema::new("builder_skill", FieldType::String)
                    .with_title("builder_skill", "制作者技能"),
            )
            .add_field(FieldSchema::new("desc", FieldType::String).with_title("desc", "制作描述"))
    }

    fn mapping_rules() -> Vec<crate::mapping::FieldMappingRule<Self>> {
        use crate::mapping::{FieldMapping, FieldMappingRule, MergeStrategy};

        vec![
            FieldMappingRule {
                target_field: "recipe_name".to_string(),
                mapping: FieldMapping::Direct { source_field: "name".to_string() },
                merge_strategy: MergeStrategy::Overwrite,
            },
            FieldMappingRule {
                target_field: "ingredient1".to_string(),
                mapping: FieldMapping::Computed {
                    compute: |recipe| {
                        recipe
                            .ingredients
                            .get(0)
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
                            .get(0)
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
                mapping: FieldMapping::Direct { source_field: "tech".to_string() },
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
                mapping: FieldMapping::Default { default: serde_json::Value::Null },
                merge_strategy: MergeStrategy::Overwrite,
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
            _ => None,
        }
    }
}
