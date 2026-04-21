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
    pub override_numtogive_fn: Option<String>,
    pub is_crafting_station: Option<bool>,
    pub icon_atlas: Option<String>,
    pub icon_image: Option<String>,
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
}

impl RecipeContext {
    pub fn new() -> Self {
        let mut ctx = Self {
            recipes: Vec::new(),
            prototyper_defs: Vec::new(),
            tech_constants: std::collections::HashMap::new(),
            variables: std::collections::HashMap::new(),
        };
        ctx.init_tech_constants();
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

    pub fn resolve_tech(&self, tech_expr: &str) -> String {
        if let Some(resolved) = self.tech_constants.get(tech_expr) {
            return resolved.clone();
        }
        tech_expr.to_string()
    }
}
