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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ingredient_new() {
        let ing = Ingredient::new("rope".to_string(), 2);
        assert_eq!(ing.item, "rope");
        assert_eq!(ing.amount, 2);
        assert!(ing.atlas.is_none());
        assert!(ing.image.is_none());
    }

    #[test]
    fn test_ingredient_with_atlas() {
        let ing = Ingredient::new("rope".to_string(), 1)
            .with_atlas("images/inventoryimages.xml".to_string());
        assert_eq!(ing.atlas, Some("images/inventoryimages.xml".to_string()));
    }

    #[test]
    fn test_ingredient_with_image() {
        let ing = Ingredient::new("rope".to_string(), 1).with_image("rope.tex".to_string());
        assert_eq!(ing.image, Some("rope.tex".to_string()));
    }

    #[test]
    fn test_ingredient_with_both() {
        let ing = Ingredient::new("rope".to_string(), 1)
            .with_atlas("images/inventoryimages.xml".to_string())
            .with_image("rope.tex".to_string());
        assert_eq!(ing.atlas, Some("images/inventoryimages.xml".to_string()));
        assert_eq!(ing.image, Some("rope.tex".to_string()));
    }

    #[test]
    fn test_ingredient_serialization() {
        let ing = Ingredient::new("goldnugget".to_string(), 5);
        let json = serde_json::to_string(&ing).unwrap();
        let deserialized: Ingredient = serde_json::from_str(&json).unwrap();
        assert_eq!(ing.item, deserialized.item);
        assert_eq!(ing.amount, deserialized.amount);
    }

    #[test]
    fn test_ingredient_clone() {
        let ing = Ingredient::new("rope".to_string(), 3);
        let cloned = ing.clone();
        assert_eq!(ing.item, cloned.item);
        assert_eq!(ing.amount, cloned.amount);
    }
}
