# PARSER MODULE

**Scope:** Game data parsers only. Lua AST (full_moon), PO text (nom). No wiki, mapping, or CLI concerns.

## STRUCTURE

```
src/parser/
├── mod.rs               # Re-exports: LuaParser, PoParser, RecipeParser,
│                        #   PrefabOverrideParser + convenience fns
│                        #   (parse_recipes_from_file, parse_prefab_overrides)
├── anim_override.rs     # AnimState 符号重映射提取（Tier A/C）：OverrideSymbol/
│                        #   OverrideSkinSymbol/ClearOverrideSymbol 常量调用（static）
│                        #   + 变量追踪（local 常量 / `..` 拼接 / `or` 回退 / alias
│                        #   receiver → resolved）→ SymbolRemapIndex
├── clothing_overrides.rs # clothing.lua CLOTHING 数据表（Tier B）：symbol_overrides/
│                        #   by_character/skintype 变体 → resolve_overrides() 复刻
│                        #   skinner.lua 的 src_sym 合成规则
├── lua.rs               # LuaParser — 3 generic AST queries:
│                        #   locate_variable, locate_field_assignment,
│                        #   locate_variable_range. Returns VariableLocation/
│                        #   FieldLocation/VariableRange structs.
├── po.rs                # PoParser — nom-based gettext PO parser.
│                        #   Returns PoFile with header + Vec<PoEntry>.
├── strings_data.rs      # parse_strings_module — 模块:<V> Strings 桶页
│                        #   `return {...}` 数据解析(full_moon) + Lua 字符串
│                        #   反转义(unescape_lua_string)。
├── recipe.rs            # RecipeParser — parses Recipe{} calls from Lua.
│                        #   Expands for-loops (numeric + generic/ipairs)
│                        #   at parse time. Uses RecipeContext for
│                        #   tech/ingredient constant resolution.
│                        #   Convenience: parse_recipes_from_file/str.
├── skilltree/          # 技能树提取（mod/scan/eval/cond/analyze）
└── prefab_override/     # Prefab name override extraction from Lua.
    ├── mod.rs           #   Re-exports: PrefabOverrideParser, types
    ├── parser/          #   PrefabOverrideParser（mod/analysis/deep 机械拆分）
    │   ├── mod.rs       #     入口 parse()/collect_definitions + tests
    │   ├── analysis.rs  #     工厂/表/SetPrefabName 分析
    │   └── deep.rs      #     跨函数边界深度解析
    └── types.rs         #   SourceLocation, PrefabNameOverride,
                         #     OverrideValue (Static/Dynamic/Unknown),
                         #     FunctionInfo, VariableValue
```

## WHERE TO LOOK

| Task | Location | Notes |
|------|----------|-------|
| Extract symbol remap calls | `anim_override.rs` → `parse_anim_overrides_in` | Only string-literal args; groups via `SymbolRemapIndex::from_calls` |
| Extract clothing symbol tables | `clothing_overrides.rs` → `parse_clothing_overrides` | `CLOTHING = {...}`; resolve via `ClothingEntry::resolve_overrides(name, character, skintype)` |
| Add a new Lua data parser | `src/parser/<name>.rs` | Full_moon AST, re-export in mod.rs |
| Query Lua var/field locations | `lua.rs` → `LuaParser::locate_*` | Generic, returns byte-offset locations |
| Parse .po translation files | `po.rs` → `PoParser::parse` | Nom combinators, returns PoFile |
| 解析 模块:Strings 桶页 | `strings_data.rs` → `parse_strings_module` | 纯数据 `return {...}`；支持字符串/角色表与 Lua 转义还原 |
| Tweak recipe parsing | `recipe.rs` → `RecipeParser` | Handles for-loop expansion; check extract_recipes() |
| Fix prefab name extraction | `prefab_override/parser/{mod,analysis,deep}.rs` | 跨函数深度解析在 `deep.rs`；工厂/表分析在 `analysis.rs`；examples 指纹测试钉住行为 |
| Add a new override value variant | `prefab_override/types.rs` | `OverrideValue` enum |
| 技能树提取 | `skilltree/` | `mod.rs` 入口/types，`scan.rs` 扫描，`eval.rs` 数值/字符串，`cond.rs` 条件翻译，`analyze.rs` 汇总 |

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
