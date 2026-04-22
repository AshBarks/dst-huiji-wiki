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

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_entry(msgctxt: Option<&str>, msgid: &str, msgstr: &str) -> PoEntry {
        PoEntry {
            msgctxt: msgctxt.map(|s| s.to_string()),
            msgid: msgid.to_string(),
            msgstr: msgstr.to_string(),
            comment: None,
        }
    }

    #[test]
    fn test_category_names() {
        let entry = create_test_entry(Some("STRINGS.NAMES.ABIGAIL"), "Abigail", "阿比盖尔");
        assert_eq!(entry.category(), Some("NAMES"));
    }

    #[test]
    fn test_category_actions() {
        let entry = create_test_entry(Some("STRINGS.ACTIONS.ABANDON"), "Abandon", "遗弃");
        assert_eq!(entry.category(), Some("ACTIONS"));
    }

    #[test]
    fn test_category_characters() {
        let entry = create_test_entry(Some("STRINGS.CHARACTERS.GENERIC.DESCRIBE"), "test", "测试");
        assert_eq!(entry.category(), Some("CHARACTERS"));
    }

    #[test]
    fn test_category_recipe_desc() {
        let entry = create_test_entry(Some("STRINGS.RECIPE_DESC.AXE"), "Axe", "斧头");
        assert_eq!(entry.category(), Some("RECIPE_DESC"));
    }

    #[test]
    fn test_category_ui() {
        let entry = create_test_entry(Some("STRINGS.UI.CRAFTING"), "Crafting", "制作");
        assert_eq!(entry.category(), Some("UI"));
    }

    #[test]
    fn test_category_none() {
        let entry = create_test_entry(Some("STRINGS.OTHER.CATEGORY"), "test", "测试");
        assert_eq!(entry.category(), None);
    }

    #[test]
    fn test_category_no_msgctxt() {
        let entry = create_test_entry(None, "test", "测试");
        assert_eq!(entry.category(), None);
    }

    #[test]
    fn test_entity_name() {
        let entry = create_test_entry(Some("STRINGS.NAMES.ABIGAIL"), "Abigail", "阿比盖尔");
        let result = entry.entity_name();
        assert_eq!(result, Some(("ABIGAIL", "阿比盖尔")));
    }

    #[test]
    fn test_entity_name_not_names() {
        let entry = create_test_entry(Some("STRINGS.ACTIONS.ABANDON"), "Abandon", "遗弃");
        assert_eq!(entry.entity_name(), None);
    }

    #[test]
    fn test_entity_name_no_msgctxt() {
        let entry = create_test_entry(None, "test", "测试");
        assert_eq!(entry.entity_name(), None);
    }

    #[test]
    fn test_po_file_new() {
        let file = PoFile::new();
        assert!(file.header.is_none());
        assert!(file.entries.is_empty());
    }

    #[test]
    fn test_po_file_filter_by_category() {
        let mut file = PoFile::new();
        file.entries.push(create_test_entry(
            Some("STRINGS.NAMES.ABIGAIL"),
            "Abigail",
            "阿比盖尔",
        ));
        file.entries.push(create_test_entry(
            Some("STRINGS.NAMES.WAXWELL"),
            "Maxwell",
            "麦斯威尔",
        ));
        file.entries.push(create_test_entry(
            Some("STRINGS.ACTIONS.ABANDON"),
            "Abandon",
            "遗弃",
        ));

        let names = file.filter_by_category("NAMES");
        assert_eq!(names.len(), 2);

        let actions = file.filter_by_category("ACTIONS");
        assert_eq!(actions.len(), 1);

        let characters = file.filter_by_category("CHARACTERS");
        assert_eq!(characters.len(), 0);
    }

    #[test]
    fn test_po_file_get_entity_names() {
        let mut file = PoFile::new();
        file.entries.push(create_test_entry(
            Some("STRINGS.NAMES.ABIGAIL"),
            "Abigail",
            "阿比盖尔",
        ));
        file.entries.push(create_test_entry(
            Some("STRINGS.NAMES.WAXWELL"),
            "Maxwell",
            "麦斯威尔",
        ));
        file.entries.push(create_test_entry(
            Some("STRINGS.ACTIONS.ABANDON"),
            "Abandon",
            "遗弃",
        ));

        let names = file.get_entity_names();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&("ABIGAIL", "阿比盖尔")));
        assert!(names.contains(&("WAXWELL", "麦斯威尔")));
    }

    #[test]
    fn test_po_entry_serialization() {
        let entry = create_test_entry(Some("STRINGS.NAMES.TEST"), "Test", "测试");
        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: PoEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(entry.msgid, deserialized.msgid);
        assert_eq!(entry.msgstr, deserialized.msgstr);
    }
}
