use crate::mapping::{
    FieldMapping, FieldMappingRule, FieldSchema, FieldType, MergeStrategy, Schema, WikiMapper,
};
use crate::models::PoEntry;

pub struct PoEntryMapper;

impl WikiMapper for PoEntry {
    fn schema() -> Schema {
        Schema::new()
            .add_field(
                FieldSchema::new("id", FieldType::String)
                    .with_title("id", "")
                    .required(),
            )
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

    fn mapping_rules() -> Vec<FieldMappingRule<Self>> {
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
                mapping: FieldMapping::Direct {
                    source_field: "msgstr".to_string(),
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
            FieldMappingRule {
                target_field: "name_en".to_string(),
                mapping: FieldMapping::Direct {
                    source_field: "msgid".to_string(),
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
            FieldMappingRule {
                target_field: "item_img1".to_string(),
                mapping: FieldMapping::Computed {
                    compute: |entry| serde_json::Value::String(format!("{}.png", entry.msgid)),
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
            "msgctxt" => self
                .msgctxt
                .as_ref()
                .map(|s| serde_json::Value::String(s.clone())),
            "msgid" => Some(serde_json::Value::String(self.msgid.clone())),
            "msgstr" => Some(serde_json::Value::String(self.msgstr.clone())),
            "comment" => self
                .comment
                .as_ref()
                .map(|s| serde_json::Value::String(s.clone())),
            _ => None,
        }
    }
}
