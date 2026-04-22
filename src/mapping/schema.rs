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

    pub fn find_record_by_field(&self, field_name: &str, value: &serde_json::Value) -> Option<&Vec<serde_json::Value>> {
        let field_idx = self.schema.fields.iter().position(|f| f.name == field_name)?;
        self.data.iter().find(|record| {
            record.get(field_idx).map_or(false, |v| v == value)
        })
    }

    pub fn find_record_idx_by_field(&self, field_name: &str, value: &serde_json::Value) -> Option<usize> {
        let field_idx = self.schema.fields.iter().position(|f| f.name == field_name)?;
        self.data.iter().position(|record| {
            record.get(field_idx).map_or(false, |v| v == value)
        })
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
