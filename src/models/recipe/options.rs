use serde::{Deserialize, Serialize};

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
