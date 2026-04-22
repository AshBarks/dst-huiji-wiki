use serde::{Deserialize, Serialize};

mod context;
mod ingredient;
mod options;
mod prototyper;

pub use context::RecipeContext;
pub use ingredient::Ingredient;
pub use options::RecipeOptions;
pub use prototyper::PrototyperDef;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recipe_new() {
        let ingredients = vec![Ingredient::new("rope".to_string(), 1)];
        let recipe = Recipe::new(
            "lighter".to_string(),
            ingredients.clone(),
            "TECH.NONE".to_string(),
        );

        assert_eq!(recipe.name, "lighter");
        assert_eq!(recipe.ingredients.len(), 1);
        assert_eq!(recipe.tech, "TECH.NONE");
        assert!(recipe.source_file.is_none());
        assert!(recipe.source_line.is_none());
    }

    #[test]
    fn test_recipe_with_options() {
        let mut options = RecipeOptions::default();
        options.builder_tag = Some("pyromaniac".to_string());
        options.numtogive = Some(2);

        let recipe = Recipe::new("lighter".to_string(), vec![], "TECH.NONE".to_string())
            .with_options(options.clone());

        assert_eq!(recipe.options.builder_tag, Some("pyromaniac".to_string()));
        assert_eq!(recipe.options.numtogive, Some(2));
    }

    #[test]
    fn test_recipe_with_source() {
        let recipe = Recipe::new("lighter".to_string(), vec![], "TECH.NONE".to_string())
            .with_source("recipes.lua".to_string(), 42);

        assert_eq!(recipe.source_file, Some("recipes.lua".to_string()));
        assert_eq!(recipe.source_line, Some(42));
    }

    #[test]
    fn test_recipe_serialization() {
        let ingredients = vec![
            Ingredient::new("rope".to_string(), 1),
            Ingredient::new("goldnugget".to_string(), 2),
        ];
        let recipe = Recipe::new("lighter".to_string(), ingredients, "TECH.NONE".to_string());

        let json = serde_json::to_string(&recipe).unwrap();
        let deserialized: Recipe = serde_json::from_str(&json).unwrap();

        assert_eq!(recipe.name, deserialized.name);
        assert_eq!(recipe.ingredients.len(), deserialized.ingredients.len());
        assert_eq!(recipe.tech, deserialized.tech);
    }

    #[test]
    fn test_recipe_clone() {
        let recipe = Recipe::new("lighter".to_string(), vec![], "TECH.NONE".to_string());
        let cloned = recipe.clone();

        assert_eq!(recipe.name, cloned.name);
        assert_eq!(recipe.tech, cloned.tech);
    }
}
