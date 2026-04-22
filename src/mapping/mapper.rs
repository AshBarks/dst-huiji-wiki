use super::schema::{Schema, WikiJsonData};

pub type JsonValue = serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MergePriority {
    NewData,
    HistoricalData,
}

pub type MergeFn = fn(&JsonValue, &JsonValue) -> JsonValue;

#[derive(Clone)]
pub enum MergeStrategy {
    Overwrite,
    PreserveHistory,
    Merge { priority: MergePriority },
    Custom(MergeFn),
}

impl PartialEq for MergeStrategy {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (MergeStrategy::Overwrite, MergeStrategy::Overwrite) => true,
            (MergeStrategy::PreserveHistory, MergeStrategy::PreserveHistory) => true,
            (MergeStrategy::Merge { priority: a }, MergeStrategy::Merge { priority: b }) => a == b,
            (MergeStrategy::Custom(a), MergeStrategy::Custom(b)) => std::ptr::fn_addr_eq(*a, *b),
            _ => false,
        }
    }
}

impl std::fmt::Debug for MergeStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MergeStrategy::Overwrite => write!(f, "Overwrite"),
            MergeStrategy::PreserveHistory => write!(f, "PreserveHistory"),
            MergeStrategy::Merge { priority } => {
                f.debug_struct("Merge").field("priority", priority).finish()
            }
            MergeStrategy::Custom(_) => write!(f, "Custom(<function>)"),
        }
    }
}

#[derive(Clone)]
pub enum FieldMapping<T> {
    Direct {
        source_field: String,
    },
    Transformed {
        source_field: String,
        transformer: fn(&T) -> JsonValue,
    },
    Computed {
        compute: fn(&T) -> JsonValue,
    },
    Constant {
        value: JsonValue,
    },
    Default {
        default: JsonValue,
    },
    Ignored,
}

impl<T> std::fmt::Debug for FieldMapping<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FieldMapping::Direct { source_field } => f
                .debug_struct("Direct")
                .field("source_field", source_field)
                .finish(),
            FieldMapping::Transformed { source_field, .. } => f
                .debug_struct("Transformed")
                .field("source_field", source_field)
                .finish_non_exhaustive(),
            FieldMapping::Computed { .. } => write!(f, "Computed(<function>)"),
            FieldMapping::Constant { value } => {
                f.debug_struct("Constant").field("value", value).finish()
            }
            FieldMapping::Default { default } => {
                f.debug_struct("Default").field("default", default).finish()
            }
            FieldMapping::Ignored => write!(f, "Ignored"),
        }
    }
}

#[derive(Clone)]
pub struct FieldMappingRule<T> {
    pub target_field: String,
    pub mapping: FieldMapping<T>,
    pub merge_strategy: MergeStrategy,
}

impl<T> std::fmt::Debug for FieldMappingRule<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FieldMappingRule")
            .field("target_field", &self.target_field)
            .field("mapping", &self.mapping)
            .field("merge_strategy", &self.merge_strategy)
            .finish()
    }
}

pub trait WikiMapper: Sized {
    fn schema() -> Schema;

    fn mapping_rules() -> Vec<FieldMappingRule<Self>>;

    fn key_field() -> &'static str;

    fn get_field_value(&self, field_name: &str) -> Option<JsonValue>;

    fn to_wiki_record(&self) -> Vec<JsonValue> {
        let rules = Self::mapping_rules();
        let schema = Self::schema();

        schema
            .fields
            .iter()
            .map(|field| {
                if let Some(rule) = rules.iter().find(|r| r.target_field == field.name) {
                    match &rule.mapping {
                        FieldMapping::Direct { source_field } => {
                            self.get_field_value(source_field).unwrap_or_else(|| {
                                field.default_value.clone().unwrap_or(JsonValue::Null)
                            })
                        }
                        FieldMapping::Transformed { transformer, .. } => transformer(self),
                        FieldMapping::Computed { compute } => compute(self),
                        FieldMapping::Constant { value } => value.clone(),
                        FieldMapping::Default { default } => default.clone(),
                        FieldMapping::Ignored => {
                            field.default_value.clone().unwrap_or(JsonValue::Null)
                        }
                    }
                } else {
                    field.default_value.clone().unwrap_or(JsonValue::Null)
                }
            })
            .collect()
    }

    fn find_historical_record<'a>(
        new_record: &[JsonValue],
        historical_data: &'a WikiJsonData,
    ) -> Option<&'a Vec<JsonValue>> {
        let key_field = Self::key_field();
        let key_idx = historical_data
            .schema
            .fields
            .iter()
            .position(|f| f.name == key_field)?;
        let new_key = new_record.get(key_idx)?;

        historical_data
            .data
            .iter()
            .find(|record| record.get(key_idx) == Some(new_key))
    }

    fn merge_record_with_history(
        new_record: &mut Vec<JsonValue>,
        historical_record: &[JsonValue],
        schema: &Schema,
    ) {
        let rules = Self::mapping_rules();

        for (idx, field) in schema.fields.iter().enumerate() {
            if let Some(rule) = rules.iter().find(|r| r.target_field == field.name) {
                let new_value = &new_record[idx];
                let historical_value = &historical_record[idx];

                new_record[idx] = match &rule.merge_strategy {
                    MergeStrategy::Overwrite => new_value.clone(),
                    MergeStrategy::PreserveHistory => {
                        if historical_value.is_null() {
                            new_value.clone()
                        } else {
                            historical_value.clone()
                        }
                    }
                    MergeStrategy::Merge { priority } => match priority {
                        MergePriority::NewData => new_value.clone(),
                        MergePriority::HistoricalData => {
                            if historical_value.is_null() {
                                new_value.clone()
                            } else {
                                historical_value.clone()
                            }
                        }
                    },
                    MergeStrategy::Custom(merge_fn) => merge_fn(new_value, historical_value),
                };
            }
        }
    }

    fn merge_with_history(new_data: &mut WikiJsonData, historical_data: &WikiJsonData) {
        let schema = Self::schema();

        for new_record in &mut new_data.data {
            if let Some(historical_record) =
                Self::find_historical_record(new_record, historical_data)
            {
                Self::merge_record_with_history(new_record, historical_record, &schema);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_priority() {
        let new_data = MergePriority::NewData;
        let historical = MergePriority::HistoricalData;

        match new_data {
            MergePriority::NewData => {}
            _ => panic!("Expected NewData"),
        }

        match historical {
            MergePriority::HistoricalData => {}
            _ => panic!("Expected HistoricalData"),
        }
    }

    #[test]
    fn test_merge_strategy_debug() {
        let overwrite = MergeStrategy::Overwrite;
        let preserve = MergeStrategy::PreserveHistory;
        let merge = MergeStrategy::Merge {
            priority: MergePriority::NewData,
        };
        let custom = MergeStrategy::Custom(|a, _| a.clone());

        assert!(format!("{:?}", overwrite).contains("Overwrite"));
        assert!(format!("{:?}", preserve).contains("PreserveHistory"));
        assert!(format!("{:?}", merge).contains("Merge"));
        assert!(format!("{:?}", custom).contains("Custom"));
    }

    #[test]
    fn test_merge_strategy_clone() {
        let strategy = MergeStrategy::Overwrite;
        let cloned = strategy.clone();
        assert!(matches!(cloned, MergeStrategy::Overwrite));
    }

    #[test]
    fn test_field_mapping_debug() {
        let direct: FieldMapping<String> = FieldMapping::Direct {
            source_field: "test".to_string(),
        };
        let computed: FieldMapping<String> = FieldMapping::Computed {
            compute: |_| JsonValue::Null,
        };
        let constant: FieldMapping<String> = FieldMapping::Constant {
            value: JsonValue::String("test".to_string()),
        };
        let ignored: FieldMapping<String> = FieldMapping::Ignored;

        assert!(format!("{:?}", direct).contains("Direct"));
        assert!(format!("{:?}", computed).contains("Computed"));
        assert!(format!("{:?}", constant).contains("Constant"));
        assert!(format!("{:?}", ignored).contains("Ignored"));
    }

    #[test]
    fn test_field_mapping_clone() {
        let mapping: FieldMapping<String> = FieldMapping::Direct {
            source_field: "test".to_string(),
        };
        let cloned = mapping.clone();
        match cloned {
            FieldMapping::Direct { source_field } => assert_eq!(source_field, "test"),
            _ => panic!("Expected Direct mapping"),
        }
    }

    #[test]
    fn test_field_mapping_rule_debug() {
        let rule: FieldMappingRule<String> = FieldMappingRule {
            target_field: "id".to_string(),
            mapping: FieldMapping::Direct {
                source_field: "name".to_string(),
            },
            merge_strategy: MergeStrategy::Overwrite,
        };
        let debug_str = format!("{:?}", rule);
        assert!(debug_str.contains("id"));
        assert!(debug_str.contains("Direct"));
        assert!(debug_str.contains("Overwrite"));
    }

    #[test]
    fn test_field_mapping_rule_clone() {
        let rule: FieldMappingRule<String> = FieldMappingRule {
            target_field: "id".to_string(),
            mapping: FieldMapping::Direct {
                source_field: "name".to_string(),
            },
            merge_strategy: MergeStrategy::Overwrite,
        };
        let cloned = rule.clone();
        assert_eq!(cloned.target_field, "id");
    }

    #[test]
    fn test_merge_strategy_overwrite() {
        let strategy = MergeStrategy::Overwrite;
        let new_val = JsonValue::String("new".to_string());

        let result = match strategy {
            MergeStrategy::Overwrite => new_val.clone(),
            _ => panic!("Expected Overwrite"),
        };
        assert_eq!(result, JsonValue::String("new".to_string()));
    }

    #[test]
    fn test_merge_strategy_preserve_history() {
        let new_val = JsonValue::String("new".to_string());
        let old_val = JsonValue::String("old".to_string());
        let null_val = JsonValue::Null;

        let result = match MergeStrategy::PreserveHistory {
            MergeStrategy::PreserveHistory => {
                if old_val.is_null() {
                    new_val.clone()
                } else {
                    old_val.clone()
                }
            }
            _ => panic!("Expected PreserveHistory"),
        };
        assert_eq!(result, JsonValue::String("old".to_string()));

        let result_null = match MergeStrategy::PreserveHistory {
            MergeStrategy::PreserveHistory => {
                if null_val.is_null() {
                    new_val.clone()
                } else {
                    null_val.clone()
                }
            }
            _ => panic!("Expected PreserveHistory"),
        };
        assert_eq!(result_null, JsonValue::String("new".to_string()));
    }

    #[test]
    fn test_merge_strategy_custom() {
        let merge_fn: MergeFn = |new_val, old_val| {
            if let (JsonValue::String(a), JsonValue::String(b)) = (new_val, old_val) {
                JsonValue::String(format!("{}+{}", a, b))
            } else {
                new_val.clone()
            }
        };

        let strategy = MergeStrategy::Custom(merge_fn);
        let new_val = JsonValue::String("new".to_string());
        let old_val = JsonValue::String("old".to_string());

        let result = match strategy {
            MergeStrategy::Custom(f) => f(&new_val, &old_val),
            _ => panic!("Expected Custom"),
        };
        assert_eq!(result, JsonValue::String("new+old".to_string()));
    }
}
