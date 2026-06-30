# PARSER MODULE

**Scope:** Game data parsers only. Lua AST (full_moon), PO text (nom). No wiki, mapping, or CLI concerns.

## STRUCTURE

```
src/parser/
├── mod.rs               # Re-exports: LuaParser, PoParser, RecipeParser,
│                        #   PrefabOverrideParser + convenience fns
│                        #   (parse_recipes_from_file, parse_prefab_overrides)
├── lua.rs               # LuaParser — 3 generic AST queries:
│                        #   locate_variable, locate_field_assignment,
│                        #   locate_variable_range. Returns VariableLocation/
│                        #   FieldLocation/VariableRange structs.
├── po.rs                # PoParser — nom-based gettext PO parser.
│                        #   Returns PoFile with header + Vec<PoEntry>.
├── recipe.rs            # RecipeParser — parses Recipe{} calls from Lua.
│                        #   Expands for-loops (numeric + generic/ipairs)
│                        #   at parse time. Uses RecipeContext for
│                        #   tech/ingredient constant resolution.
│                        #   Convenience: parse_recipes_from_file/str.
└── prefab_override/     # Prefab name override extraction from Lua.
    ├── mod.rs           #   Re-exports: PrefabOverrideParser, types
    ├── parser.rs        #   PrefabOverrideParser — 2657 lines (largest
    │                    #     file in project). Deep AST walk across
    │                    #     function boundaries. Tracks local vars +
    │                    #     params to resolve prefab name overrides.
    │                    #     Handles factory patterns, table.insert,
    │                    #     ipairs, control flow branches.
    │                    #     Also exports parse_prefab_overrides().
    ├── types.rs         #   SourceLocation, PrefabNameOverride,
    │                    #     OverrideValue (Static/Dynamic/Unknown),
    │                    #     FunctionInfo, VariableValue
    └── control_flow.rs  #   visit_control_flow_blocks — single helper
                         #     that walks if/while/repeat/for blocks.
```

## WHERE TO LOOK

| Task | Location | Notes |
|------|----------|-------|
| Add a new Lua data parser | `src/parser/<name>.rs` | Full_moon AST, re-export in mod.rs |
| Query Lua var/field locations | `lua.rs` → `LuaParser::locate_*` | Generic, returns byte-offset locations |
| Parse .po translation files | `po.rs` → `PoParser::parse` | Nom combinators, returns PoFile |
| Tweak recipe parsing | `recipe.rs` → `RecipeParser` | Handles for-loop expansion; check extract_recipes() |
| Fix prefab name extraction | `prefab_override/parser.rs` | Most complex file. Search by: collect_definitions, walk_block, resolve_override |
| Add a new override value variant | `prefab_override/types.rs` | `OverrideValue` enum |
| Modify control flow traversal | `prefab_override/control_flow.rs` | Single function, called from parser.rs |

## CONVENTIONS

- **full_moon with lua52** — all Lua parsers use this (DST scripts are Lua 5.2)
- **nom = "8"** — only po.rs uses nom; combinators, not macros
- **Parser structs hold context, free fns for one-shots** — RecipeParser/PrefabOverrideParser store AST + state; PoParser/LuaParser are stateless
- **Result<T> everywhere** — crate error type, never unwrap outside tests
- **byte-offset locations** — VariableLocation/FieldLocation/SourceLocation all use start_byte/end_byte for source slicing

## ANTI-PATTERNS (THIS MODULE)

- **DO NOT** add regex-based Lua parsing. Use full_moon AST traversal only.
- **DO NOT** add `unwrap()` in parser logic — always propagate errors via `?`.
- **DO NOT** put parser tests in separate `tests/` files. Inline `#[cfg(test)] mod tests` only.
- **DO NOT** mix wiki API or mapping logic into parsers. Parsers return models, nothing else.
- **DO NOT** add unsafe blocks. AST walking needs no unsafe.
