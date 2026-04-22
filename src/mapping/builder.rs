use super::mapper::{FieldMapping, FieldMappingRule, JsonValue, MergeFn, MergePriority, MergeStrategy};
use super::schema::{FieldSchema, FieldType, Schema};

pub struct MappingBuilder<T> {
    schema: Schema,
    rules: Vec<FieldMappingRule<T>>,
    key_field: Option<String>,
}

impl<T> MappingBuilder<T> {
    pub fn new() -> Self {
        Self {
            schema: Schema::new(),
            rules: Vec::new(),
            key_field: None,
        }
    }

    pub fn field(mut self, name: &str, field_type: FieldType) -> Self {
        self.schema.fields.push(FieldSchema::new(name, field_type));
        self
    }

    pub fn with_title(mut self, en: &str, zh: &str) -> Self {
        if let Some(field) = self.schema.fields.last_mut() {
            field.title = Some(super::schema::FieldTitle {
                en: en.to_string(),
                zh: zh.to_string(),
            });
        }
        self
    }

    pub fn required(mut self) -> Self {
        if let Some(field) = self.schema.fields.last_mut() {
            field.required = true;
        }
        self
    }

    pub fn with_default(mut self, value: JsonValue) -> Self {
        if let Some(field) = self.schema.fields.last_mut() {
            field.default_value = Some(value);
        }
        self
    }

    pub fn key_field(mut self, field_name: &str) -> Self {
        self.key_field = Some(field_name.to_string());
        self
    }

    pub fn map_direct(mut self, target: &str, source: &str) -> Self {
        self.rules.push(FieldMappingRule {
            target_field: target.to_string(),
            mapping: FieldMapping::Direct { source_field: source.to_string() },
            merge_strategy: MergeStrategy::Overwrite,
        });
        self
    }

    pub fn map_transformed(mut self, target: &str, source: &str, transformer: fn(&T) -> JsonValue) -> Self {
        self.rules.push(FieldMappingRule {
            target_field: target.to_string(),
            mapping: FieldMapping::Transformed {
                source_field: source.to_string(),
                transformer,
            },
            merge_strategy: MergeStrategy::Overwrite,
        });
        self
    }

    pub fn map_computed(mut self, target: &str, compute: fn(&T) -> JsonValue) -> Self {
        self.rules.push(FieldMappingRule {
            target_field: target.to_string(),
            mapping: FieldMapping::Computed { compute },
            merge_strategy: MergeStrategy::Overwrite,
        });
        self
    }

    pub fn map_constant(mut self, target: &str, value: JsonValue) -> Self {
        self.rules.push(FieldMappingRule {
            target_field: target.to_string(),
            mapping: FieldMapping::Constant { value },
            merge_strategy: MergeStrategy::Overwrite,
        });
        self
    }

    pub fn map_default(mut self, target: &str, default: JsonValue) -> Self {
        self.rules.push(FieldMappingRule {
            target_field: target.to_string(),
            mapping: FieldMapping::Default { default },
            merge_strategy: MergeStrategy::Overwrite,
        });
        self
    }

    pub fn map_ignored(mut self, target: &str) -> Self {
        self.rules.push(FieldMappingRule {
            target_field: target.to_string(),
            mapping: FieldMapping::Ignored,
            merge_strategy: MergeStrategy::Overwrite,
        });
        self
    }

    pub fn with_overwrite(mut self, target: &str) -> Self {
        if let Some(rule) = self.rules.iter_mut().find(|r| r.target_field == target) {
            rule.merge_strategy = MergeStrategy::Overwrite;
        }
        self
    }

    pub fn with_preserve_history(mut self, target: &str) -> Self {
        if let Some(rule) = self.rules.iter_mut().find(|r| r.target_field == target) {
            rule.merge_strategy = MergeStrategy::PreserveHistory;
        }
        self
    }

    pub fn with_merge_priority(mut self, target: &str, priority: MergePriority) -> Self {
        if let Some(rule) = self.rules.iter_mut().find(|r| r.target_field == target) {
            rule.merge_strategy = MergeStrategy::Merge { priority };
        }
        self
    }

    pub fn with_custom_merge(mut self, target: &str, merge_fn: MergeFn) -> Self {
        if let Some(rule) = self.rules.iter_mut().find(|r| r.target_field == target) {
            rule.merge_strategy = MergeStrategy::Custom(merge_fn);
        }
        self
    }

    pub fn build(self) -> (Schema, Vec<FieldMappingRule<T>>, String) {
        let key = self.key_field.unwrap_or_else(|| {
            self.schema
                .fields
                .first()
                .map(|f| f.name.clone())
                .unwrap_or_default()
        });
        (self.schema, self.rules, key)
    }
}

impl<T> Default for MappingBuilder<T> {
    fn default() -> Self {
        Self::new()
    }
}

pub struct SchemaBuilder {
    schema: Schema,
}

impl SchemaBuilder {
    pub fn new() -> Self {
        Self { schema: Schema::new() }
    }

    pub fn string_field(mut self, name: &str) -> Self {
        self.schema.fields.push(FieldSchema::new(name, FieldType::String));
        self
    }

    pub fn integer_field(mut self, name: &str) -> Self {
        self.schema.fields.push(FieldSchema::new(name, FieldType::Number));
        self
    }

    pub fn float_field(mut self, name: &str) -> Self {
        self.schema.fields.push(FieldSchema::new(name, FieldType::Number));
        self
    }

    pub fn boolean_field(mut self, name: &str) -> Self {
        self.schema.fields.push(FieldSchema::new(name, FieldType::Boolean));
        self
    }

    pub fn array_field(mut self, name: &str, element_type: FieldType) -> Self {
        self.schema
            .fields
            .push(FieldSchema::new(name, FieldType::Array(Box::new(element_type))));
        self
    }

    pub fn object_field(mut self, name: &str) -> Self {
        self.schema.fields.push(FieldSchema::new(name, FieldType::Object));
        self
    }

    pub fn with_title(mut self, en: &str, zh: &str) -> Self {
        if let Some(field) = self.schema.fields.last_mut() {
            field.title = Some(super::schema::FieldTitle {
                en: en.to_string(),
                zh: zh.to_string(),
            });
        }
        self
    }

    pub fn required(mut self) -> Self {
        if let Some(field) = self.schema.fields.last_mut() {
            field.required = true;
        }
        self
    }

    pub fn with_default(mut self, value: JsonValue) -> Self {
        if let Some(field) = self.schema.fields.last_mut() {
            field.default_value = Some(value);
        }
        self
    }

    pub fn build(self) -> Schema {
        self.schema
    }
}

impl Default for SchemaBuilder {
    fn default() -> Self {
        Self::new()
    }
}
