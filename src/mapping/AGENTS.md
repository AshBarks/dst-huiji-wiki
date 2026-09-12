# src/mapping/ AGENTS.md

**Scope**: data to wiki mapping framework. Converts parsed game data (Recipe, PoEntry) into wiki JSON schema output.

## STRUCTURE
```
src/mapping/
├── mod.rs          # Module decls + re-exports (converter mod after pub use -- unusual but valid)
├── mapper.rs       # WikiMapper trait + FieldMapping/MergeStrategy enums
├── schema.rs       # Schema, FieldSchema, WikiSchema, WikiJsonData structs
├── converter.rs    # WikiDataConverter + DataDiffReport + PoLookupTable（按字段名对齐 diff/merge）
└── mappers/
    ├── mod.rs      # Re-exports PoEntryMapper, RecipeMapper
    ├── po.rs       # PoEntryMapper -- WikiMapper for PoEntry (114 lines)
    └── recipe.rs   # RecipeMapper -- WikiMapper for Recipe (470 lines)
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Understand core mapping interface | `mapper.rs` | WikiMapper trait, FieldMapping enum (6 variants), MergeStrategy enum (4 variants) |
| Define a mapper | `mappers/*.rs` | `impl WikiMapper for Model` 手写 `mapping_rules()` + `key_field()` + `merge_record_with_history()`（已无 Builder） |
| Define wiki JSON output shape | `schema.rs` | WikiJsonData (license/description/sources/schema/data) + Schema/FieldSchema |
| Orchestrate full pipeline | `converter.rs` | WikiDataConverter: parse then convert then compare then merge |
| Track data differences | `converter.rs` | `DataDiffReport` + `compare_data(new, historical, key_field)`；字段按名对齐 |
| Map Recipe to wiki JSON | `mappers/recipe.rs` | 470 lines, complex field mappings + PO description lookup |
| Map PoEntry to wiki JSON | `mappers/po.rs` | 114 lines, simpler mapping for name translation data |

## CONVENTIONS
- **Mapper as adapter struct**: PoEntryMapper and RecipeMapper are zero-sized structs. WikiMapper is impl'd on the model type (PoEntry, Recipe), not on the mapper struct
- **Merge strategy per field**: Each `FieldMappingRule` carries its own `MergeStrategy`
- **Concrete mappers are thin**: Schema + rules defined in impl blocks; actual conversion uses WikiMapper default methods (to_wiki_record, merge_with_history)
- **Field-name alignment**: `converter::compare_data` / `compare_records` / `merge_new_records` locate the key by `T::key_field()` and align fields by name on both schemas（两侧字段顺序可不同，历史侧缺字段视为 Null）；调用方必须显式传 `T::key_field()`

## ANTI-PATTERNS
- **DO NOT** put conversion logic in mappers/ -- converters handle orchestration, mappers define schema+rules only
- **DO NOT** import from `crate::commands` or `crate::main` -- mapping is a lib module, commands is binary-only
- **DO NOT** impl WikiMapper on the mapper struct (e.g., `impl WikiMapper for RecipeMapper`) -- impl on the model type instead
- **DO NOT** add serde derives to FieldType, FieldSchema, or Schema -- they are internal types; only WikiSchema and WikiJsonData serialize
- **DO NOT** use wildcard imports in mod.rs -- all re-exports are explicit from each submodule
