mod builder;
mod converter;
mod mapper;
mod schema;

pub use builder::{MappingBuilder, SchemaBuilder};
pub use converter::{
    compare_and_report, compare_data, merge_new_records, replace_records, DataDiffReport,
    FieldChange, PoLookupTable, RecordChange, WikiDataConverter,
};
pub use mapper::{
    FieldMapping, FieldMappingRule, JsonValue, MergeFn, MergePriority, MergeStrategy, WikiMapper,
};
pub use schema::{
    FieldSchema, FieldType, FieldTitle, Schema, WikiFieldSchema, WikiJsonData, WikiSchema,
};
