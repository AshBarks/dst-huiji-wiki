use super::{PrototyperDef, Recipe};
use std::collections::HashMap;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct RecipeContext {
    pub recipes: Vec<Recipe>,
    pub prototyper_defs: Vec<PrototyperDef>,
    pub tech_constants: HashMap<String, String>,
    pub variables: HashMap<String, String>,
    pub character_ingredients: HashMap<String, String>,
    pub tech_ingredients: HashMap<String, String>,
    pub tuning_constants: HashMap<String, i32>,
}

impl RecipeContext {
    pub fn new() -> Self {
        let mut ctx = Self {
            recipes: Vec::new(),
            prototyper_defs: Vec::new(),
            tech_constants: HashMap::new(),
            variables: HashMap::new(),
            character_ingredients: HashMap::new(),
            tech_ingredients: HashMap::new(),
            tuning_constants: HashMap::new(),
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
