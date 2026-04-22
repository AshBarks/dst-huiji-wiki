use serde::{Deserialize, Serialize};

pub type FieldName = String;

#[derive(Debug, Clone, PartialEq)]
pub enum FieldType {
    String,
    Number,
    Boolean,
    Array(Box<FieldType>),
    Object,
}

impl FieldType {
    pub fn to_json_type(&self) -> &'static str {
        match self {
            FieldType::String => "string",
            FieldType::Number => "number",
            FieldType::Boolean => "boolean",
            FieldType::Array(_) => "array",
            FieldType::Object => "object",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldTitle {
    pub en: String,
    pub zh: String,
}

#[derive(Debug, Clone)]
pub struct FieldSchema {
    pub name: FieldName,
    pub field_type: FieldType,
    pub title: Option<FieldTitle>,
    pub required: bool,
    pub default_value: Option<serde_json::Value>,
}

impl FieldSchema {
    pub fn new(name: impl Into<String>, field_type: FieldType) -> Self {
        Self {
            name: name.into(),
            field_type,
            title: None,
            required: false,
            default_value: None,
        }
    }

    pub fn with_title(mut self, en: impl Into<String>, zh: impl Into<String>) -> Self {
        self.title = Some(FieldTitle {
            en: en.into(),
            zh: zh.into(),
        });
        self
    }

    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    pub fn with_default(mut self, value: serde_json::Value) -> Self {
        self.default_value = Some(value);
        self
    }

    pub fn to_wiki_field(&self) -> WikiFieldSchema {
        WikiFieldSchema {
            name: self.name.clone(),
            field_type: self.field_type.to_json_type().to_string(),
            title: self.title.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikiFieldSchema {
    pub name: String,
    #[serde(rename = "type")]
    pub field_type: String,
    pub title: Option<FieldTitle>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikiSchema {
    pub fields: Vec<WikiFieldSchema>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikiJsonData {
    pub license: String,
    pub description: serde_json::Value,
    pub sources: String,
    pub schema: WikiSchema,
    pub data: Vec<Vec<serde_json::Value>>,
}

impl WikiJsonData {
    pub fn new(sources: String, schema: WikiSchema, description: serde_json::Value) -> Self {
        Self {
            license: "CC0-1.0".to_string(),
            description,
            sources,
            schema,
            data: Vec::new(),
        }
    }

    pub fn add_record(&mut self, record: Vec<serde_json::Value>) {
        self.data.push(record);
    }

    pub fn find_record_by_field(
        &self,
        field_name: &str,
        value: &serde_json::Value,
    ) -> Option<&Vec<serde_json::Value>> {
        let field_idx = self
            .schema
            .fields
            .iter()
            .position(|f| f.name == field_name)?;
        self.data
            .iter()
            .find(|record| record.get(field_idx) == Some(value))
    }

    pub fn find_record_idx_by_field(
        &self,
        field_name: &str,
        value: &serde_json::Value,
    ) -> Option<usize> {
        let field_idx = self
            .schema
            .fields
            .iter()
            .position(|f| f.name == field_name)?;
        self.data
            .iter()
            .position(|record| record.get(field_idx) == Some(value))
    }
}

pub struct Schema {
    pub fields: Vec<FieldSchema>,
}

impl Schema {
    pub fn new() -> Self {
        Self { fields: Vec::new() }
    }

    pub fn add_field(mut self, field: FieldSchema) -> Self {
        self.fields.push(field);
        self
    }

    pub fn field_names(&self) -> Vec<&str> {
        self.fields.iter().map(|f| f.name.as_str()).collect()
    }

    pub fn to_wiki_schema(&self) -> WikiSchema {
        WikiSchema {
            fields: self.fields.iter().map(|f| f.to_wiki_field()).collect(),
        }
    }
}

impl Default for Schema {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_type_to_json_type() {
        assert_eq!(FieldType::String.to_json_type(), "string");
        assert_eq!(FieldType::Number.to_json_type(), "number");
        assert_eq!(FieldType::Boolean.to_json_type(), "boolean");
        assert_eq!(
            FieldType::Array(Box::new(FieldType::String)).to_json_type(),
            "array"
        );
        assert_eq!(FieldType::Object.to_json_type(), "object");
    }

    #[test]
    fn test_field_schema_new() {
        let field = FieldSchema::new("test_field", FieldType::String);
        assert_eq!(field.name, "test_field");
        assert_eq!(field.field_type, FieldType::String);
        assert!(field.title.is_none());
        assert!(!field.required);
        assert!(field.default_value.is_none());
    }

    #[test]
    fn test_field_schema_with_title() {
        let field = FieldSchema::new("test", FieldType::String).with_title("Test", "测试");
        assert!(field.title.is_some());
        let title = field.title.unwrap();
        assert_eq!(title.en, "Test");
        assert_eq!(title.zh, "测试");
    }

    #[test]
    fn test_field_schema_required() {
        let field = FieldSchema::new("test", FieldType::String).required();
        assert!(field.required);
    }

    #[test]
    fn test_field_schema_with_default() {
        let field = FieldSchema::new("test", FieldType::String)
            .with_default(serde_json::json!("default_value"));
        assert_eq!(
            field.default_value,
            Some(serde_json::json!("default_value"))
        );
    }

    #[test]
    fn test_field_schema_to_wiki_field() {
        let field = FieldSchema::new("id", FieldType::String)
            .with_title("ID", "标识")
            .required();
        let wiki_field = field.to_wiki_field();
        assert_eq!(wiki_field.name, "id");
        assert_eq!(wiki_field.field_type, "string");
        assert!(wiki_field.title.is_some());
    }

    #[test]
    fn test_schema_new() {
        let schema = Schema::new();
        assert!(schema.fields.is_empty());
    }

    #[test]
    fn test_schema_add_field() {
        let schema = Schema::new()
            .add_field(FieldSchema::new("id", FieldType::String))
            .add_field(FieldSchema::new("name", FieldType::String));
        assert_eq!(schema.fields.len(), 2);
    }

    #[test]
    fn test_schema_field_names() {
        let schema = Schema::new()
            .add_field(FieldSchema::new("id", FieldType::String))
            .add_field(FieldSchema::new("name", FieldType::String));
        let names = schema.field_names();
        assert_eq!(names, vec!["id", "name"]);
    }

    #[test]
    fn test_schema_to_wiki_schema() {
        let schema = Schema::new()
            .add_field(FieldSchema::new("id", FieldType::String))
            .add_field(FieldSchema::new("count", FieldType::Number));
        let wiki_schema = schema.to_wiki_schema();
        assert_eq!(wiki_schema.fields.len(), 2);
        assert_eq!(wiki_schema.fields[0].field_type, "string");
        assert_eq!(wiki_schema.fields[1].field_type, "number");
    }

    #[test]
    fn test_wiki_json_data_new() {
        let schema = WikiSchema { fields: vec![] };
        let data = WikiJsonData::new(
            "test source".to_string(),
            schema,
            serde_json::json!({"zh": "测试"}),
        );
        assert_eq!(data.license, "CC0-1.0");
        assert_eq!(data.sources, "test source");
        assert!(data.data.is_empty());
    }

    #[test]
    fn test_wiki_json_data_add_record() {
        let schema = WikiSchema {
            fields: vec![WikiFieldSchema {
                name: "id".to_string(),
                field_type: "string".to_string(),
                title: None,
            }],
        };
        let mut data = WikiJsonData::new("source".to_string(), schema, serde_json::json!({}));
        data.add_record(vec![serde_json::json!("test_id")]);
        assert_eq!(data.data.len(), 1);
    }

    #[test]
    fn test_wiki_json_data_find_record_by_field() {
        let schema = WikiSchema {
            fields: vec![
                WikiFieldSchema {
                    name: "id".to_string(),
                    field_type: "string".to_string(),
                    title: None,
                },
                WikiFieldSchema {
                    name: "name".to_string(),
                    field_type: "string".to_string(),
                    title: None,
                },
            ],
        };
        let mut data = WikiJsonData::new("source".to_string(), schema, serde_json::json!({}));
        data.add_record(vec![serde_json::json!("id1"), serde_json::json!("name1")]);
        data.add_record(vec![serde_json::json!("id2"), serde_json::json!("name2")]);

        let found = data.find_record_by_field("id", &serde_json::json!("id1"));
        assert!(found.is_some());
        let record = found.unwrap();
        assert_eq!(record[0], serde_json::json!("id1"));

        let not_found = data.find_record_by_field("id", &serde_json::json!("id3"));
        assert!(not_found.is_none());
    }

    #[test]
    fn test_wiki_json_data_find_record_idx_by_field() {
        let schema = WikiSchema {
            fields: vec![WikiFieldSchema {
                name: "id".to_string(),
                field_type: "string".to_string(),
                title: None,
            }],
        };
        let mut data = WikiJsonData::new("source".to_string(), schema, serde_json::json!({}));
        data.add_record(vec![serde_json::json!("id1")]);
        data.add_record(vec![serde_json::json!("id2")]);

        assert_eq!(
            data.find_record_idx_by_field("id", &serde_json::json!("id1")),
            Some(0)
        );
        assert_eq!(
            data.find_record_idx_by_field("id", &serde_json::json!("id2")),
            Some(1)
        );
        assert_eq!(
            data.find_record_idx_by_field("id", &serde_json::json!("id3")),
            None
        );
    }

    #[test]
    fn test_wiki_json_data_serialization() {
        let schema = WikiSchema {
            fields: vec![WikiFieldSchema {
                name: "id".to_string(),
                field_type: "string".to_string(),
                title: Some(FieldTitle {
                    en: "ID".to_string(),
                    zh: "标识".to_string(),
                }),
            }],
        };
        let data = WikiJsonData::new(
            "source".to_string(),
            schema,
            serde_json::json!({"zh": "测试数据"}),
        );

        let json = serde_json::to_string(&data).unwrap();
        let deserialized: WikiJsonData = serde_json::from_str(&json).unwrap();
        assert_eq!(data.sources, deserialized.sources);
        assert_eq!(data.schema.fields.len(), deserialized.schema.fields.len());
    }
}
