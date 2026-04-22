use super::mapper::{
    FieldMapping, FieldMappingRule, JsonValue, MergeFn, MergePriority, MergeStrategy,
};
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
            mapping: FieldMapping::Direct {
                source_field: source.to_string(),
            },
            merge_strategy: MergeStrategy::Overwrite,
        });
        self
    }

    pub fn map_transformed(
        mut self,
        target: &str,
        source: &str,
        transformer: fn(&T) -> JsonValue,
    ) -> Self {
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
        Self {
            schema: Schema::new(),
        }
    }

    pub fn string_field(mut self, name: &str) -> Self {
        self.schema
            .fields
            .push(FieldSchema::new(name, FieldType::String));
        self
    }

    pub fn integer_field(mut self, name: &str) -> Self {
        self.schema
            .fields
            .push(FieldSchema::new(name, FieldType::Number));
        self
    }

    pub fn float_field(mut self, name: &str) -> Self {
        self.schema
            .fields
            .push(FieldSchema::new(name, FieldType::Number));
        self
    }

    pub fn boolean_field(mut self, name: &str) -> Self {
        self.schema
            .fields
            .push(FieldSchema::new(name, FieldType::Boolean));
        self
    }

    pub fn array_field(mut self, name: &str, element_type: FieldType) -> Self {
        self.schema.fields.push(FieldSchema::new(
            name,
            FieldType::Array(Box::new(element_type)),
        ));
        self
    }

    pub fn object_field(mut self, name: &str) -> Self {
        self.schema
            .fields
            .push(FieldSchema::new(name, FieldType::Object));
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

#[cfg(test)]
mod tests {
    use super::*;

    struct TestItem {
        _name: String,
        _value: i32,
    }

    #[test]
    fn test_mapping_builder_new() {
        let builder: MappingBuilder<TestItem> = MappingBuilder::new();
        assert!(builder.schema.fields.is_empty());
        assert!(builder.rules.is_empty());
        assert!(builder.key_field.is_none());
    }

    #[test]
    fn test_mapping_builder_field() {
        let builder: MappingBuilder<TestItem> = MappingBuilder::new()
            .field("id", FieldType::String)
            .field("count", FieldType::Number);
        assert_eq!(builder.schema.fields.len(), 2);
    }

    #[test]
    fn test_mapping_builder_with_title() {
        let builder: MappingBuilder<TestItem> = MappingBuilder::new()
            .field("id", FieldType::String)
            .with_title("ID", "标识");
        assert!(builder.schema.fields[0].title.is_some());
    }

    #[test]
    fn test_mapping_builder_required() {
        let builder: MappingBuilder<TestItem> = MappingBuilder::new()
            .field("id", FieldType::String)
            .required();
        assert!(builder.schema.fields[0].required);
    }

    #[test]
    fn test_mapping_builder_with_default() {
        let builder: MappingBuilder<TestItem> = MappingBuilder::new()
            .field("count", FieldType::Number)
            .with_default(JsonValue::Number(0.into()));
        assert_eq!(
            builder.schema.fields[0].default_value,
            Some(JsonValue::Number(0.into()))
        );
    }

    #[test]
    fn test_mapping_builder_key_field() {
        let builder: MappingBuilder<TestItem> = MappingBuilder::new()
            .field("id", FieldType::String)
            .key_field("id");
        assert_eq!(builder.key_field, Some("id".to_string()));
    }

    #[test]
    fn test_mapping_builder_map_direct() {
        let builder: MappingBuilder<TestItem> = MappingBuilder::new()
            .field("id", FieldType::String)
            .map_direct("id", "name");
        assert_eq!(builder.rules.len(), 1);
        match &builder.rules[0].mapping {
            FieldMapping::Direct { source_field } => {
                assert_eq!(source_field, "name");
            }
            _ => panic!("Expected Direct mapping"),
        }
    }

    #[test]
    fn test_mapping_builder_map_computed() {
        let builder: MappingBuilder<TestItem> = MappingBuilder::new()
            .field("computed", FieldType::Number)
            .map_computed("computed", |_item: &TestItem| JsonValue::Number(42.into()));
        assert_eq!(builder.rules.len(), 1);
        match &builder.rules[0].mapping {
            FieldMapping::Computed { .. } => {}
            _ => panic!("Expected Computed mapping"),
        }
    }

    #[test]
    fn test_mapping_builder_map_constant() {
        let builder: MappingBuilder<TestItem> = MappingBuilder::new()
            .field("status", FieldType::String)
            .map_constant("status", JsonValue::String("active".to_string()));
        assert_eq!(builder.rules.len(), 1);
        match &builder.rules[0].mapping {
            FieldMapping::Constant { value } => {
                assert_eq!(value, &JsonValue::String("active".to_string()));
            }
            _ => panic!("Expected Constant mapping"),
        }
    }

    #[test]
    fn test_mapping_builder_map_default() {
        let builder: MappingBuilder<TestItem> = MappingBuilder::new()
            .field("count", FieldType::Number)
            .map_default("count", JsonValue::Number(1.into()));
        assert_eq!(builder.rules.len(), 1);
        match &builder.rules[0].mapping {
            FieldMapping::Default { default } => {
                assert_eq!(default, &JsonValue::Number(1.into()));
            }
            _ => panic!("Expected Default mapping"),
        }
    }

    #[test]
    fn test_mapping_builder_map_ignored() {
        let builder: MappingBuilder<TestItem> = MappingBuilder::new()
            .field("ignored", FieldType::String)
            .map_ignored("ignored");
        assert_eq!(builder.rules.len(), 1);
        match &builder.rules[0].mapping {
            FieldMapping::Ignored => {}
            _ => panic!("Expected Ignored mapping"),
        }
    }

    #[test]
    fn test_mapping_builder_with_overwrite() {
        let builder: MappingBuilder<TestItem> = MappingBuilder::new()
            .field("id", FieldType::String)
            .map_direct("id", "name")
            .with_overwrite("id");
        assert_eq!(builder.rules[0].merge_strategy, MergeStrategy::Overwrite);
    }

    #[test]
    fn test_mapping_builder_with_preserve_history() {
        let builder: MappingBuilder<TestItem> = MappingBuilder::new()
            .field("id", FieldType::String)
            .map_direct("id", "name")
            .with_preserve_history("id");
        assert_eq!(
            builder.rules[0].merge_strategy,
            MergeStrategy::PreserveHistory
        );
    }

    #[test]
    fn test_mapping_builder_with_merge_priority() {
        let builder: MappingBuilder<TestItem> = MappingBuilder::new()
            .field("id", FieldType::String)
            .map_direct("id", "name")
            .with_merge_priority("id", MergePriority::NewData);
        match &builder.rules[0].merge_strategy {
            MergeStrategy::Merge { priority } => {
                assert_eq!(*priority, MergePriority::NewData);
            }
            _ => panic!("Expected Merge strategy"),
        }
    }

    #[test]
    fn test_mapping_builder_build() {
        let (schema, rules, key) = MappingBuilder::<TestItem>::new()
            .field("id", FieldType::String)
            .field("name", FieldType::String)
            .key_field("id")
            .map_direct("id", "name")
            .build();
        assert_eq!(schema.fields.len(), 2);
        assert_eq!(rules.len(), 1);
        assert_eq!(key, "id");
    }

    #[test]
    fn test_mapping_builder_build_default_key() {
        let (schema, _rules, key) = MappingBuilder::<TestItem>::new()
            .field("first_field", FieldType::String)
            .build();
        assert_eq!(schema.fields.len(), 1);
        assert_eq!(key, "first_field");
    }

    #[test]
    fn test_schema_builder_new() {
        let builder = SchemaBuilder::new();
        assert!(builder.schema.fields.is_empty());
    }

    #[test]
    fn test_schema_builder_string_field() {
        let schema = SchemaBuilder::new().string_field("name").build();
        assert_eq!(schema.fields.len(), 1);
        assert_eq!(schema.fields[0].field_type, FieldType::String);
    }

    #[test]
    fn test_schema_builder_integer_field() {
        let schema = SchemaBuilder::new().integer_field("count").build();
        assert_eq!(schema.fields[0].field_type, FieldType::Number);
    }

    #[test]
    fn test_schema_builder_float_field() {
        let schema = SchemaBuilder::new().float_field("price").build();
        assert_eq!(schema.fields[0].field_type, FieldType::Number);
    }

    #[test]
    fn test_schema_builder_boolean_field() {
        let schema = SchemaBuilder::new().boolean_field("active").build();
        assert_eq!(schema.fields[0].field_type, FieldType::Boolean);
    }

    #[test]
    fn test_schema_builder_array_field() {
        let schema = SchemaBuilder::new()
            .array_field("tags", FieldType::String)
            .build();
        match &schema.fields[0].field_type {
            FieldType::Array(inner) => assert_eq!(**inner, FieldType::String),
            _ => panic!("Expected Array type"),
        }
    }

    #[test]
    fn test_schema_builder_object_field() {
        let schema = SchemaBuilder::new().object_field("metadata").build();
        assert_eq!(schema.fields[0].field_type, FieldType::Object);
    }

    #[test]
    fn test_schema_builder_chained() {
        let schema = SchemaBuilder::new()
            .string_field("id")
            .with_title("ID", "标识")
            .required()
            .integer_field("count")
            .with_default(JsonValue::Number(0.into()))
            .build();
        assert_eq!(schema.fields.len(), 2);
        assert!(schema.fields[0].required);
        assert!(schema.fields[0].title.is_some());
        assert!(schema.fields[1].default_value.is_some());
    }
}
