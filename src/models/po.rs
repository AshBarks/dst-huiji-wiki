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

impl crate::mapping::WikiMapper for PoEntry {
    fn schema() -> crate::mapping::Schema {
        use crate::mapping::{FieldSchema, FieldType, Schema};

        Schema::new()
            .add_field(FieldSchema::new("id", FieldType::String).with_title("id", "").required())
            .add_field(
                FieldSchema::new("name_cn", FieldType::String)
                    .with_title("name_cn", "")
                    .required(),
            )
            .add_field(
                FieldSchema::new("name_en", FieldType::String)
                    .with_title("name_en", "")
                    .required(),
            )
            .add_field(
                FieldSchema::new("item_img1", FieldType::String)
                    .with_title("item_img1", "")
                    .with_default(serde_json::Value::String(String::new())),
            )
    }

    fn mapping_rules() -> Vec<crate::mapping::FieldMappingRule<Self>> {
        use crate::mapping::{FieldMapping, FieldMappingRule, MergeStrategy};

        vec![
            FieldMappingRule {
                target_field: "id".to_string(),
                mapping: FieldMapping::Transformed {
                    source_field: "msgctxt".to_string(),
                    transformer: |entry| {
                        entry
                            .msgctxt
                            .as_ref()
                            .and_then(|ctx| ctx.strip_prefix("STRINGS.NAMES."))
                            .map(|s| serde_json::Value::String(s.to_lowercase()))
                            .unwrap_or(serde_json::Value::Null)
                    },
                },
                merge_strategy: MergeStrategy::Overwrite,
            },
            FieldMappingRule {
                target_field: "name_cn".to_string(),
                mapping: FieldMapping::Direct { source_field: "msgstr".to_string() },
                merge_strategy: MergeStrategy::Custom(|new_val, historical_val| {
                    if let serde_json::Value::String(s) = historical_val {
                        if !s.is_empty() {
                            return historical_val.clone();
                        }
                    }
                    new_val.clone()
                }),
            },
            FieldMappingRule {
                target_field: "name_en".to_string(),
                mapping: FieldMapping::Direct { source_field: "msgid".to_string() },
                merge_strategy: MergeStrategy::Custom(|new_val, historical_val| {
                    if let serde_json::Value::String(s) = historical_val {
                        if !s.is_empty() {
                            return historical_val.clone();
                        }
                    }
                    new_val.clone()
                }),
            },
            FieldMappingRule {
                target_field: "item_img1".to_string(),
                mapping: FieldMapping::Computed {
                    compute: |entry| {
                        serde_json::Value::String(format!("{}.png", entry.msgid))
                    },
                },
                merge_strategy: MergeStrategy::Custom(|new_val, historical_val| {
                    if let serde_json::Value::String(s) = historical_val {
                        if !s.is_empty() {
                            return historical_val.clone();
                        }
                    }
                    new_val.clone()
                }),
            },
        ]
    }

    fn key_field() -> &'static str {
        "id"
    }

    fn get_field_value(&self, field_name: &str) -> Option<serde_json::Value> {
        match field_name {
            "msgctxt" => self.msgctxt.as_ref().map(|s| serde_json::Value::String(s.clone())),
            "msgid" => Some(serde_json::Value::String(self.msgid.clone())),
            "msgstr" => Some(serde_json::Value::String(self.msgstr.clone())),
            "comment" => self.comment.as_ref().map(|s| serde_json::Value::String(s.clone())),
            _ => None,
        }
    }
}
