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
        Self { entries: HashMap::new() }
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
        Self { po_lookup: Some(PoLookupTable::from_po_entries(entries)) }
    }

    pub fn with_po_lookup(po_lookup: PoLookupTable) -> Self {
        Self { po_lookup: Some(po_lookup) }
    }

    pub fn po_lookup(&self) -> Option<&PoLookupTable> {
        self.po_lookup.as_ref()
    }

    pub fn convert_to_wiki_json<T: WikiMapper>(&self, items: &[T], sources: &str) -> WikiJsonData {
        let schema = T::schema();
        let wiki_schema = schema.to_wiki_schema();

        let data = items.iter().map(|item| item.to_wiki_record()).collect();

        WikiJsonData { sources: sources.to_string(), schema: wiki_schema, data }
    }

    pub fn convert_recipes(
        &self,
        recipes: &[crate::models::Recipe],
        sources: &str,
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

        WikiJsonData { sources: sources.to_string(), schema: wiki_schema, data }
    }

    pub fn convert_with_history<T: WikiMapper>(
        &self,
        items: &[T],
        sources: &str,
        historical_data: &WikiJsonData,
    ) -> WikiJsonData {
        let mut wiki_data = self.convert_to_wiki_json(items, sources);
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
            report.push_str("\n");
        }

        if !self.deleted.is_empty() {
            report.push_str("删除记录:\n");
            for key in &self.deleted {
                report.push_str(&format!("  - {}\n", format_value(key)));
            }
            report.push_str("\n");
        }

        if !self.modified.is_empty() {
            report.push_str("修改记录:\n");
            for record_change in &self.modified {
                report.push_str(&format!("  * {}:\n", format_value(&record_change.key)));
                for change in &record_change.changes {
                    report.push_str(&format!(
                        "      {}\n",
                        format_field_change(&change.field_name, &change.old_value, &change.new_value)
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
    let new_keys: Vec<Value> = new_data.data.iter().map(|r| r.first().cloned().unwrap_or(Value::Null)).collect();

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
                new_data.schema.fields.first().map(|f| f.name.as_str()).unwrap_or(""),
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
    let field_names: Vec<&str> = new_data.schema.fields.iter().map(|f| f.name.as_str()).collect();
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

        if let Some(existing_idx) =
            result.find_record_idx_by_field(T::key_field(), &new_record[0])
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

pub fn replace_records<T: WikiMapper>(items: &[T], sources: &str) -> WikiJsonData {
    WikiDataConverter::new().convert_to_wiki_json(items, sources)
}
