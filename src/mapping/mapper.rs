use super::schema::{Schema, WikiJsonData};

pub type JsonValue = serde_json::Value;

#[derive(Debug, Clone, Copy)]
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

impl std::fmt::Debug for MergeStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MergeStrategy::Overwrite => write!(f, "Overwrite"),
            MergeStrategy::PreserveHistory => write!(f, "PreserveHistory"),
            MergeStrategy::Merge { priority } => f.debug_struct("Merge").field("priority", priority).finish(),
            MergeStrategy::Custom(_) => write!(f, "Custom(<function>)"),
        }
    }
}

#[derive(Clone)]
pub enum FieldMapping<T> {
    Direct { source_field: String },
    Transformed { source_field: String, transformer: fn(&T) -> JsonValue },
    Computed { compute: fn(&T) -> JsonValue },
    Constant { value: JsonValue },
    Default { default: JsonValue },
    Ignored,
}

impl<T> std::fmt::Debug for FieldMapping<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FieldMapping::Direct { source_field } => {
                f.debug_struct("Direct").field("source_field", source_field).finish()
            }
            FieldMapping::Transformed { source_field, .. } => {
                f.debug_struct("Transformed").field("source_field", source_field).finish_non_exhaustive()
            }
            FieldMapping::Computed { .. } => write!(f, "Computed(<function>)"),
            FieldMapping::Constant { value } => f.debug_struct("Constant").field("value", value).finish(),
            FieldMapping::Default { default } => f.debug_struct("Default").field("default", default).finish(),
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
                        FieldMapping::Direct { source_field } => self
                            .get_field_value(source_field)
                            .unwrap_or_else(|| field.default_value.clone().unwrap_or(JsonValue::Null)),
                        FieldMapping::Transformed { transformer, .. } => transformer(self),
                        FieldMapping::Computed { compute } => compute(self),
                        FieldMapping::Constant { value } => value.clone(),
                        FieldMapping::Default { default } => default.clone(),
                        FieldMapping::Ignored => field.default_value.clone().unwrap_or(JsonValue::Null),
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
        let key_idx = historical_data.schema.fields.iter().position(|f| f.name == key_field)?;
        let new_key = new_record.get(key_idx)?;

        historical_data.data.iter().find(|record| record.get(key_idx) == Some(new_key))
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
            if let Some(historical_record) = Self::find_historical_record(new_record, historical_data) {
                Self::merge_record_with_history(new_record, historical_record, &schema);
            }
        }
    }
}
