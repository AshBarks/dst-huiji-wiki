# src/mapping/ AGENTS.md

**Scope**: data to wiki mapping framework. Converts parsed game data (Recipe, PoEntry) into wiki JSON schema output.

## STRUCTURE
```
src/mapping/
├── mod.rs          # Module decls + re-exports (converter mod after pub use -- unusual but valid)
├── mapper.rs       # WikiMapper trait + FieldMapping/MergeStrategy enums
├── builder.rs      # MappingBuilder<T> + SchemaBuilder fluent APIs
├── schema.rs       # Schema, FieldSchema, WikiSchema, WikiJsonData structs
├── converter.rs    # WikiDataConverter + DataDiffReport + PoLookupTable
└── mappers/
    ├── mod.rs      # Re-exports PoEntryMapper, RecipeMapper
    ├── po.rs       # PoEntryMapper -- WikiMapper for PoEntry (114 lines)
    └── recipe.rs   # RecipeMapper -- WikiMapper for Recipe (470 lines)
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Understand core mapping interface | `mapper.rs` | WikiMapper trait, FieldMapping enum (6 variants), MergeStrategy enum (4 variants) |
| Build a mapper definition | `builder.rs` | MappingBuilder<T> fluent API (6 map_* + 4 with_* merge control methods) |
| Build a wiki schema | `builder.rs` | SchemaBuilder fluent API (string/integer/float/boolean/array/object field methods) |
| Define wiki JSON output shape | `schema.rs` | WikiJsonData (license/description/sources/schema/data) + Schema/FieldSchema |
| Orchestrate full pipeline | `converter.rs` | WikiDataConverter: parse then convert then compare then merge |
| Track data differences | `converter.rs` | DataDiffReport (added/deleted/modified records) + compare_data() |
| Map Recipe to wiki JSON | `mappers/recipe.rs` | 470 lines, complex field mappings + PO description lookup |
| Map PoEntry to wiki JSON | `mappers/po.rs` | 114 lines, simpler mapping for name translation data |

## CONVENTIONS
- **Mapper as adapter struct**: PoEntryMapper and RecipeMapper are zero-sized structs. WikiMapper is impl'd on the model type (PoEntry, Recipe), not on the mapper struct
- **build() returns tuple**: MappingBuilder<T>::build() returns (Schema, Vec\<FieldMappingRule\<T\>\>, String) -- schema, rules, key field name
- **Merge strategy per field**: Each FieldMappingRule carries its own MergeStrategy, settable via with_* methods after the mapping call
- **Concrete mappers are thin**: Schema + rules defined in impl blocks; actual conversion uses WikiMapper default methods (to_wiki_record, merge_with_history)
- **converter mod after pub use**: mod.rs declares `mod converter;` after all `pub use` statements instead of before them

## ANTI-PATTERNS
- **DO NOT** put conversion logic in mappers/ -- converters handle orchestration, mappers define schema+rules only
- **DO NOT** import from `crate::commands` or `crate::main` -- mapping is a lib module, commands is binary-only
- **DO NOT** impl WikiMapper on the mapper struct (e.g., `impl WikiMapper for RecipeMapper`) -- impl on the model type instead
- **DO NOT** add serde derives to FieldType, FieldSchema, or Schema -- they are internal types; only WikiSchema and WikiJsonData serialize
- **DO NOT** use wildcard imports in mod.rs -- all re-exports are explicit from each submodule
