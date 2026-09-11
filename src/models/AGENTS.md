# src/models/ AGENTS.md

**Scope**: Data model structs with serde derives. No parsing, no I/O, no API calls.

## OVERVIEW
All data shapes: PO translation entries, recipe definitions, tech comparison reports. Structs + methods only.

## STRUCTURE
```
src/models/
├── mod.rs            # 5 pub mod decls, re-exports key types
├── po.rs             # PoEntry + PoFile (PO/Gettext data)
├── strings.rs        # 模块:Strings 数据:key 大写归一 + 角色表合并 + 分桶/索引 + Lua 渲染/转义 + diff
├── tech_report.rs    # TechReport (tech level diff)
└── recipe/
    ├── mod.rs        # Recipe struct + re-exports sub-models
    ├── context.rs    # RecipeContext (hardcoded game constants)
    ├── ingredient.rs # Ingredient (item + amount + atlas/image)
    ├── options.rs    # RecipeOptions (21 optional fields)
    └── prototyper.rs # PrototyperDef (crafting station)
```

## WHERE TO LOOK
| File | Types | Key info |
|------|-------|----------|
| `po.rs` | PoEntry, PoFile | category() extracts NAMES/ACTIONS/CHARACTERS/RECIPE_DESC/UI from msgctxt. entity_name() strips STRINGS.NAMES. prefix. PoFile has filter_by_category() + get_entity_names() |
| `strings.rs` | StringValue, WikiStringKey, BucketPlan, MapDiff, ... | wiki_string_key(): STRINGS. 前缀剥离 + ASCII 大写归一 + CHARACTERS.<speaker> 移入小写角色表键（GENERIC→wilson）；build_language_map/plan_equal_count/plan_with_boundaries/validate_plan/render_module/diff_values |
| `tech_report.rs` | TechReport | from_recipes() builds from Recipe slice. parse_wiki_lua_data() parses Lua 'TECH.*' entries. compare_with_wiki() computes diff. generate_report() formats text output |
| `recipe/mod.rs` | Recipe | name + ingredients(Vec<Ingredient>) + tech + options + source_file/line. Builder: with_options(), with_source() |
| `recipe/context.rs` | RecipeContext | Hardcoded maps: tech_constants (SCIENCE_ONE..SHADOW_THREE), character_ingredients, tech_ingredients, tuning_constants. Resolvers: resolve_tech(), resolve_ingredient(), resolve_tuning() |
| `recipe/ingredient.rs` | Ingredient | item + amount + optional atlas/image. Builder: with_atlas(), with_image() |
| `recipe/options.rs` | RecipeOptions | 21 Option<T> fields: builder_tag, numtogive, product, placer, image, nounlock, no_deconstruction, min_spacing, testfn, action_str, filter_text, sg_state, description, override_numtogive_fn, is_crafting_station, icon_atlas, icon_image, hint_msg, unlocks_from_skin, station_tag |
| `recipe/prototyper.rs` | PrototyperDef | name + icon_atlas + icon_image + is_crafting_station + action_str + filter_text |

## CONVENTIONS
- All structs derive serde::{Serialize, Deserialize} for JSON round-trip
- RecipeOptions uses Option<T> on every field -- Default gives empty state
- Builder pattern on sub-types (Ingredient::with_atlas, Recipe::with_options, Ingredient::with_image) for ergonomic construction
- RecipeContext contains HARDCODED game constant maps -- update manually when DST adds new tech tiers or ingredient types
- TechReport is pure computation (no I/O): parses wiki Lua from string slices, never from files
- Recipe struct lives in mod.rs, not its own file. All 4 sub-modules (context, ingredient, options, prototyper) are private, only re-exported types are public

## ANTI-PATTERNS
- DO NOT add I/O to models -- parse in parser/, persist in mapping/, keep models pure
- DO NOT add hardcoded constant maps without corresponding tests (context.rs has 6 test cases)
- DO NOT import crate::parser or crate::wiki from models/ -- breaks layering
- DO NOT make sub-module files (ingredient.rs, options.rs, etc.) pub -- re-export through recipe/mod.rs
- DO NOT add non-serde dependencies to models/ -- this crate stays lightweight
