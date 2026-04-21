use serde::{Deserialize, Serialize};

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
