use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoEntry {
    pub msgctxt: Option<String>,
    pub msgid: String,
    pub msgstr: String,
    pub comment: Option<String>,
}

impl PoEntry {
    pub fn category(&self) -> Option<&str> {
        self.msgctxt.as_ref().and_then(|ctx| {
            if ctx.starts_with("STRINGS.NAMES.") {
                Some("NAMES")
            } else if ctx.starts_with("STRINGS.ACTIONS.") {
                Some("ACTIONS")
            } else if ctx.starts_with("STRINGS.CHARACTERS.") {
                Some("CHARACTERS")
            } else if ctx.starts_with("STRINGS.RECIPE_DESC.") {
                Some("RECIPE_DESC")
            } else if ctx.starts_with("STRINGS.UI.") {
                Some("UI")
            } else {
                None
            }
        })
    }

    pub fn entity_name(&self) -> Option<(&str, &str)> {
        self.msgctxt.as_ref().and_then(|ctx| {
            ctx.strip_prefix("STRINGS.NAMES.")
                .map(|entity_code| (entity_code, self.msgstr.as_str()))
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoFile {
    pub header: Option<String>,
    pub entries: Vec<PoEntry>,
}

impl PoFile {
    pub fn new() -> Self {
        Self {
            header: None,
            entries: Vec::new(),
        }
    }

    pub fn filter_by_category(&self, category: &str) -> Vec<&PoEntry> {
        self.entries
            .iter()
            .filter(|e| e.category() == Some(category))
            .collect()
    }

    pub fn get_entity_names(&self) -> Vec<(&str, &str)> {
        self.entries
            .iter()
            .filter_map(|e| e.entity_name())
            .collect()
    }
}

impl Default for PoFile {
    fn default() -> Self {
        Self::new()
    }
}
