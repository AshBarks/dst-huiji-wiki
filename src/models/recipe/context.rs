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
            self.tech_constants
                .insert(key.to_string(), value.to_string());
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
            self.character_ingredients
                .insert(key.to_string(), value.to_string());
        }

        let tech_ingredients = [("TECH_INGREDIENT.SCULPTING", "sculpting_material")];
        for (key, value) in tech_ingredients {
            self.tech_ingredients
                .insert(key.to_string(), value.to_string());
        }
    }

    pub fn resolve_tech(&self, tech_expr: &str) -> String {
        tech_expr.to_string()
    }

    pub fn resolve_ingredient(&self, item_expr: &str) -> Result<String, String> {
        if item_expr.starts_with("CHARACTER_INGREDIENT.")
            || item_expr.starts_with("TECH_INGREDIENT.")
        {
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
        let tuning_constants = [("TUNING.EFFIGY_HEALTH_PENALTY", 40)];
        for (key, value) in tuning_constants {
            self.tuning_constants.insert(key.to_string(), value);
        }
    }

    pub fn resolve_tuning(&self, expr: &str) -> Option<i32> {
        self.tuning_constants.get(expr).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_new() {
        let ctx = RecipeContext::new();
        assert!(ctx.recipes.is_empty());
        assert!(ctx.prototyper_defs.is_empty());
        assert!(!ctx.tech_constants.is_empty());
        assert!(!ctx.character_ingredients.is_empty());
    }

    #[test]
    fn test_tech_constants_initialized() {
        let ctx = RecipeContext::new();
        assert!(ctx.tech_constants.contains_key("TECH.NONE"));
        assert!(ctx.tech_constants.contains_key("TECH.SCIENCE_ONE"));
        assert!(ctx.tech_constants.contains_key("TECH.MAGIC_TWO"));
        assert!(ctx.tech_constants.contains_key("TECH.SHADOW_THREE"));
        assert_eq!(
            ctx.tech_constants.get("TECH.NONE"),
            Some(&"NONE".to_string())
        );
    }

    #[test]
    fn test_character_ingredients_initialized() {
        let ctx = RecipeContext::new();
        assert!(ctx
            .character_ingredients
            .contains_key("CHARACTER_INGREDIENT.HEALTH"));
        assert!(ctx
            .character_ingredients
            .contains_key("CHARACTER_INGREDIENT.SANITY"));
        assert_eq!(
            ctx.character_ingredients.get("CHARACTER_INGREDIENT.HEALTH"),
            Some(&"decrease_health".to_string())
        );
    }

    #[test]
    fn test_tech_ingredients_initialized() {
        let ctx = RecipeContext::new();
        assert!(ctx
            .tech_ingredients
            .contains_key("TECH_INGREDIENT.SCULPTING"));
        assert_eq!(
            ctx.tech_ingredients.get("TECH_INGREDIENT.SCULPTING"),
            Some(&"sculpting_material".to_string())
        );
    }

    #[test]
    fn test_tuning_constants_initialized() {
        let ctx = RecipeContext::new();
        assert!(ctx
            .tuning_constants
            .contains_key("TUNING.EFFIGY_HEALTH_PENALTY"));
        assert_eq!(
            ctx.tuning_constants.get("TUNING.EFFIGY_HEALTH_PENALTY"),
            Some(&40)
        );
    }

    #[test]
    fn test_resolve_tech() {
        let ctx = RecipeContext::new();
        assert_eq!(ctx.resolve_tech("TECH.NONE"), "TECH.NONE");
        assert_eq!(ctx.resolve_tech("TECH.SCIENCE_ONE"), "TECH.SCIENCE_ONE");
    }

    #[test]
    fn test_resolve_ingredient_character() {
        let ctx = RecipeContext::new();
        let result = ctx.resolve_ingredient("CHARACTER_INGREDIENT.HEALTH");
        assert_eq!(result, Ok("decrease_health".to_string()));
    }

    #[test]
    fn test_resolve_ingredient_tech() {
        let ctx = RecipeContext::new();
        let result = ctx.resolve_ingredient("TECH_INGREDIENT.SCULPTING");
        assert_eq!(result, Ok("sculpting_material".to_string()));
    }

    #[test]
    fn test_resolve_ingredient_normal() {
        let ctx = RecipeContext::new();
        let result = ctx.resolve_ingredient("rope");
        assert_eq!(result, Ok("rope".to_string()));
    }

    #[test]
    fn test_resolve_ingredient_unknown_constant() {
        let ctx = RecipeContext::new();
        let result = ctx.resolve_ingredient("CHARACTER_INGREDIENT.UNKNOWN");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown ingredient constant"));
    }

    #[test]
    fn test_resolve_tuning() {
        let ctx = RecipeContext::new();
        assert_eq!(ctx.resolve_tuning("TUNING.EFFIGY_HEALTH_PENALTY"), Some(40));
        assert_eq!(ctx.resolve_tuning("TUNING.UNKNOWN"), None);
    }

    #[test]
    fn test_context_default() {
        let ctx = RecipeContext::default();
        assert!(ctx.recipes.is_empty());
        assert!(ctx.tech_constants.is_empty());
    }
}
