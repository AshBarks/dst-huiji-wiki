use super::mapper::WikiMapper;
use super::schema::WikiJsonData;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct PoLookupTable {
    entries: HashMap<String, String>,
}

impl PoLookupTable {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub fn from_po_entries(entries: Vec<crate::models::PoEntry>) -> Self {
        let mut table = Self::new();
        for entry in entries {
            if let Some(msgctxt) = &entry.msgctxt {
                table.entries.insert(msgctxt.clone(), entry.msgstr);
            }
        }
        table
    }

    pub fn get(&self, msgctxt: &str) -> Option<&String> {
        self.entries.get(msgctxt)
    }

    pub fn get_recipe_desc(
        &self,
        description: Option<&str>,
        recipe_name: &str,
        product: Option<&str>,
    ) -> Option<String> {
        if let Some(desc) = description {
            let key = format!("STRINGS.RECIPE_DESC.{}", desc.to_uppercase());
            return self.get(&key).cloned();
        }

        if let Some(prod) = product {
            let key = format!("STRINGS.RECIPE_DESC.{}", prod.to_uppercase());
            if let Some(s) = self.get(&key) {
                return Some(s.clone());
            }
        }

        let key = format!("STRINGS.RECIPE_DESC.{}", recipe_name.to_uppercase());
        self.get(&key).cloned()
    }
}

pub struct WikiDataConverter {
    po_lookup: Option<PoLookupTable>,
}

impl Default for WikiDataConverter {
    fn default() -> Self {
        Self::new()
    }
}

impl WikiDataConverter {
    pub fn new() -> Self {
        Self { po_lookup: None }
    }

    pub fn with_po_entries(entries: Vec<crate::models::PoEntry>) -> Self {
        Self {
            po_lookup: Some(PoLookupTable::from_po_entries(entries)),
        }
    }

    pub fn with_po_lookup(po_lookup: PoLookupTable) -> Self {
        Self {
            po_lookup: Some(po_lookup),
        }
    }

    pub fn po_lookup(&self) -> Option<&PoLookupTable> {
        self.po_lookup.as_ref()
    }

    pub fn convert_to_wiki_json<T: WikiMapper>(
        &self,
        items: &[T],
        sources: &str,
        description: serde_json::Value,
    ) -> WikiJsonData {
        let schema = T::schema();
        let wiki_schema = schema.to_wiki_schema();

        let data = items.iter().map(|item| item.to_wiki_record()).collect();

        WikiJsonData {
            license: "CC0-1.0".to_string(),
            description,
            sources: sources.to_string(),
            schema: wiki_schema,
            data,
        }
    }

    pub fn convert_recipes(
        &self,
        recipes: &[crate::models::Recipe],
        sources: &str,
        description: serde_json::Value,
    ) -> WikiJsonData {
        let schema = crate::models::Recipe::schema();
        let wiki_schema = schema.to_wiki_schema();

        let data = recipes
            .iter()
            .map(|recipe| {
                let mut record = recipe.to_wiki_record();
                if let Some(po_lookup) = &self.po_lookup {
                    let desc = po_lookup.get_recipe_desc(
                        recipe.options.description.as_deref(),
                        &recipe.name,
                        recipe.options.product.as_deref(),
                    );
                    if let Some(desc_value) = desc {
                        if record.len() > 25 {
                            record[25] = Value::String(desc_value);
                        }
                    }
                }
                record
            })
            .collect();

        WikiJsonData {
            license: "CC0-1.0".to_string(),
            description,
            sources: sources.to_string(),
            schema: wiki_schema,
            data,
        }
    }

    pub fn convert_with_history<T: WikiMapper>(
        &self,
        items: &[T],
        sources: &str,
        historical_data: &WikiJsonData,
        description: serde_json::Value,
    ) -> WikiJsonData {
        let mut wiki_data = self.convert_to_wiki_json(items, sources, description);
        T::merge_with_history(&mut wiki_data, historical_data);
        wiki_data
    }

    pub fn to_json_string(data: &WikiJsonData) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(data)
    }

    pub fn to_json_value(data: &WikiJsonData) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::to_value(data)
    }

    pub fn parse_wiki_json(json_str: &str) -> Result<WikiJsonData, serde_json::Error> {
        serde_json::from_str(json_str)
    }
}

#[derive(Debug, Clone)]
pub struct FieldChange {
    pub field_name: String,
    pub old_value: Value,
    pub new_value: Value,
}

#[derive(Debug, Clone)]
pub struct RecordChange {
    pub key: Value,
    pub changes: Vec<FieldChange>,
}

#[derive(Debug, Clone)]
pub struct DataDiffReport {
    pub added: Vec<Value>,
    pub deleted: Vec<Value>,
    pub modified: Vec<RecordChange>,
    pub total_new: usize,
    pub total_historical: usize,
}

impl DataDiffReport {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.deleted.is_empty() && self.modified.is_empty()
    }

    pub fn summary(&self) -> String {
        format!(
            "数据比较报告:\n  新增: {} 条\n  删除: {} 条\n  修改: {} 条\n  新数据总数: {} 条\n  历史数据总数: {} 条",
            self.added.len(),
            self.deleted.len(),
            self.modified.len(),
            self.total_new,
            self.total_historical
        )
    }

    pub fn detailed_report(&self, _field_names: &[&str]) -> String {
        let mut report = self.summary();
        report.push_str("\n\n");

        if !self.added.is_empty() {
            report.push_str("新增记录:\n");
            for key in &self.added {
                report.push_str(&format!("  + {}\n", format_value(key)));
            }
            report.push('\n');
        }

        if !self.deleted.is_empty() {
            report.push_str("删除记录:\n");
            for key in &self.deleted {
                report.push_str(&format!("  - {}\n", format_value(key)));
            }
            report.push('\n');
        }

        if !self.modified.is_empty() {
            report.push_str("修改记录:\n");
            for record_change in &self.modified {
                report.push_str(&format!("  * {}:\n", format_value(&record_change.key)));
                for change in &record_change.changes {
                    report.push_str(&format!(
                        "      {}\n",
                        format_field_change(
                            &change.field_name,
                            &change.old_value,
                            &change.new_value
                        )
                    ));
                }
            }
        }

        report
    }
}

fn format_value(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_string(),
        _ => value.to_string(),
    }
}

fn format_field_change(field: &str, old: &Value, new: &Value) -> String {
    format!("{}: {} => {}", field, format_value(old), format_value(new))
}

pub fn compare_data(new_data: &WikiJsonData, historical_data: &WikiJsonData) -> DataDiffReport {
    let new_keys: Vec<Value> = new_data
        .data
        .iter()
        .map(|r| r.first().cloned().unwrap_or(Value::Null))
        .collect();

    let historical_keys: Vec<Value> = historical_data
        .data
        .iter()
        .map(|r| r.first().cloned().unwrap_or(Value::Null))
        .collect();

    let added: Vec<Value> = new_keys
        .iter()
        .filter(|key| !historical_keys.contains(key))
        .cloned()
        .collect();

    let deleted: Vec<Value> = historical_keys
        .iter()
        .filter(|key| !new_keys.contains(key))
        .cloned()
        .collect();

    let modified: Vec<RecordChange> = new_data
        .data
        .iter()
        .filter_map(|new_record| {
            let key = new_record.first().cloned().unwrap_or(Value::Null);
            if let Some(historical_record) = historical_data.find_record_by_field(
                new_data
                    .schema
                    .fields
                    .first()
                    .map(|f| f.name.as_str())
                    .unwrap_or(""),
                &key,
            ) {
                let changes = compare_records(new_record, historical_record, new_data);
                if !changes.is_empty() {
                    Some(RecordChange { key, changes })
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();

    DataDiffReport {
        added,
        deleted,
        modified,
        total_new: new_data.data.len(),
        total_historical: historical_data.data.len(),
    }
}

fn compare_records(
    new_record: &[Value],
    historical_record: &[Value],
    wiki_data: &WikiJsonData,
) -> Vec<FieldChange> {
    wiki_data
        .schema
        .fields
        .iter()
        .enumerate()
        .filter_map(|(idx, field)| {
            let new_val = new_record.get(idx).unwrap_or(&Value::Null);
            let old_val = historical_record.get(idx).unwrap_or(&Value::Null);

            if new_val != old_val {
                Some(FieldChange {
                    field_name: field.name.clone(),
                    old_value: old_val.clone(),
                    new_value: new_val.clone(),
                })
            } else {
                None
            }
        })
        .collect()
}

pub fn compare_and_report(new_data: &WikiJsonData, historical_data: &WikiJsonData) -> String {
    let report = compare_data(new_data, historical_data);
    let field_names: Vec<&str> = new_data
        .schema
        .fields
        .iter()
        .map(|f| f.name.as_str())
        .collect();
    report.detailed_report(&field_names)
}

pub fn merge_new_records<T: WikiMapper>(
    new_items: &[T],
    historical_data: &WikiJsonData,
    sources: &str,
) -> WikiJsonData {
    let mut result = historical_data.clone();

    for item in new_items {
        let new_record = item.to_wiki_record();

        if let Some(existing_idx) = result.find_record_idx_by_field(T::key_field(), &new_record[0])
        {
            let schema = T::schema();
            let mut record_to_merge = new_record.clone();
            T::merge_record_with_history(&mut record_to_merge, &result.data[existing_idx], &schema);
            result.data[existing_idx] = record_to_merge;
        } else {
            result.data.push(new_record);
        }
    }

    result.sources = sources.to_string();
    result
}

pub fn replace_records<T: WikiMapper>(
    items: &[T],
    sources: &str,
    description: serde_json::Value,
) -> WikiJsonData {
    WikiDataConverter::new().convert_to_wiki_json(items, sources, description)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::PoEntry;

    fn create_po_entry(msgctxt: &str, msgstr: &str) -> PoEntry {
        PoEntry {
            msgctxt: Some(msgctxt.to_string()),
            msgid: "test".to_string(),
            msgstr: msgstr.to_string(),
            comment: None,
        }
    }

    #[test]
    fn test_po_lookup_table_new() {
        let table = PoLookupTable::new();
        assert!(table.get("any_key").is_none());
    }

    #[test]
    fn test_po_lookup_table_from_entries() {
        let entries = vec![
            create_po_entry("STRINGS.RECIPE_DESC.AXE", "斧头描述"),
            create_po_entry("STRINGS.RECIPE_DESC.PICKAXE", "镐描述"),
        ];
        let table = PoLookupTable::from_po_entries(entries);
        assert_eq!(
            table.get("STRINGS.RECIPE_DESC.AXE"),
            Some(&"斧头描述".to_string())
        );
        assert_eq!(
            table.get("STRINGS.RECIPE_DESC.PICKAXE"),
            Some(&"镐描述".to_string())
        );
    }

    #[test]
    fn test_po_lookup_table_get_recipe_desc_from_description() {
        let entries = vec![create_po_entry("STRINGS.RECIPE_DESC.AXE", "斧头描述")];
        let table = PoLookupTable::from_po_entries(entries);
        let result = table.get_recipe_desc(Some("axe"), "other", None);
        assert_eq!(result, Some("斧头描述".to_string()));
    }

    #[test]
    fn test_po_lookup_table_get_recipe_desc_from_product() {
        let entries = vec![create_po_entry("STRINGS.RECIPE_DESC.PICKAXE", "镐描述")];
        let table = PoLookupTable::from_po_entries(entries);
        let result = table.get_recipe_desc(None, "other", Some("pickaxe"));
        assert_eq!(result, Some("镐描述".to_string()));
    }

    #[test]
    fn test_po_lookup_table_get_recipe_desc_from_name() {
        let entries = vec![create_po_entry("STRINGS.RECIPE_DESC.SHOVEL", "铲子描述")];
        let table = PoLookupTable::from_po_entries(entries);
        let result = table.get_recipe_desc(None, "shovel", None);
        assert_eq!(result, Some("铲子描述".to_string()));
    }

    #[test]
    fn test_po_lookup_table_get_recipe_desc_not_found() {
        let table = PoLookupTable::new();
        let result = table.get_recipe_desc(None, "unknown", None);
        assert!(result.is_none());
    }

    #[test]
    fn test_wiki_data_converter_new() {
        let converter = WikiDataConverter::new();
        assert!(converter.po_lookup().is_none());
    }

    #[test]
    fn test_wiki_data_converter_with_po_entries() {
        let entries = vec![create_po_entry("STRINGS.RECIPE_DESC.TEST", "测试")];
        let converter = WikiDataConverter::with_po_entries(entries);
        assert!(converter.po_lookup().is_some());
    }

    #[test]
    fn test_wiki_data_converter_to_json_string() {
        let schema = super::super::schema::WikiSchema { fields: vec![] };
        let data = WikiJsonData::new("source".to_string(), schema, serde_json::json!({}));
        let json = WikiDataConverter::to_json_string(&data).unwrap();
        assert!(json.contains("CC0-1.0"));
        assert!(json.contains("source"));
    }

    #[test]
    fn test_wiki_data_converter_parse_wiki_json() {
        let json = r#"{
            "license": "CC0-1.0",
            "description": {},
            "sources": "test",
            "schema": {"fields": []},
            "data": []
        }"#;
        let data = WikiDataConverter::parse_wiki_json(json).unwrap();
        assert_eq!(data.license, "CC0-1.0");
        assert_eq!(data.sources, "test");
    }

    #[test]
    fn test_data_diff_report_is_empty() {
        let report = DataDiffReport {
            added: vec![],
            deleted: vec![],
            modified: vec![],
            total_new: 0,
            total_historical: 0,
        };
        assert!(report.is_empty());
    }

    #[test]
    fn test_data_diff_report_not_empty() {
        let report = DataDiffReport {
            added: vec![Value::String("new".to_string())],
            deleted: vec![],
            modified: vec![],
            total_new: 1,
            total_historical: 0,
        };
        assert!(!report.is_empty());
    }

    #[test]
    fn test_data_diff_report_summary() {
        let report = DataDiffReport {
            added: vec![Value::String("a".to_string())],
            deleted: vec![Value::String("b".to_string())],
            modified: vec![],
            total_new: 5,
            total_historical: 4,
        };
        let summary = report.summary();
        assert!(summary.contains("新增: 1 条"));
        assert!(summary.contains("删除: 1 条"));
        assert!(summary.contains("新数据总数: 5 条"));
    }

    #[test]
    fn test_format_value() {
        assert_eq!(format_value(&Value::String("test".to_string())), "test");
        assert_eq!(format_value(&Value::Number(42.into())), "42");
        assert_eq!(format_value(&Value::Bool(true)), "true");
        assert_eq!(format_value(&Value::Null), "null");
    }

    #[test]
    fn test_format_field_change() {
        let result = format_field_change(
            "name",
            &Value::String("old".to_string()),
            &Value::String("new".to_string()),
        );
        assert_eq!(result, "name: old => new");
    }

    #[test]
    fn test_field_change() {
        let change = FieldChange {
            field_name: "test_field".to_string(),
            old_value: Value::String("old".to_string()),
            new_value: Value::String("new".to_string()),
        };
        assert_eq!(change.field_name, "test_field");
        assert_eq!(change.old_value, Value::String("old".to_string()));
        assert_eq!(change.new_value, Value::String("new".to_string()));
    }

    #[test]
    fn test_record_change() {
        let change = RecordChange {
            key: Value::String("id1".to_string()),
            changes: vec![FieldChange {
                field_name: "name".to_string(),
                old_value: Value::String("old".to_string()),
                new_value: Value::String("new".to_string()),
            }],
        };
        assert_eq!(change.key, Value::String("id1".to_string()));
        assert_eq!(change.changes.len(), 1);
    }
}
