mod builder;
mod mapper;
mod mappers;
mod schema;

pub use builder::{MappingBuilder, SchemaBuilder};
pub use mapper::{
    FieldMapping, FieldMappingRule, JsonValue, MergeFn, MergePriority, MergeStrategy, WikiMapper,
};
pub use mappers::{PoEntryMapper, RecipeMapper};
pub use schema::{
    FieldSchema, FieldType, FieldTitle, Schema, WikiFieldSchema, WikiJsonData, WikiSchema,
};

mod converter;
pub use converter::{
    compare_and_report, compare_data, merge_new_records, replace_records, DataDiffReport,
    FieldChange, PoLookupTable, RecordChange, WikiDataConverter,
};
