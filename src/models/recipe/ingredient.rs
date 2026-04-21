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
