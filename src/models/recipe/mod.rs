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
