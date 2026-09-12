# PREFAB OVERRIDE PARSER

**Scope:** Extracts `PrefabNameOverride` entries from DST Lua scripts via deep full_moon AST walking. Formerly one 3145-line `parser.rs`; mechanically split by responsibility (behavior unchanged).

## STRUCTURE

```
prefab_override/
├── mod.rs          # Re-exports: PrefabOverrideParser,
│                   #   parse_prefab_overrides (free fn),
│                   #   PrefabNameOverride, OverrideValue, SourceLocation
├── parser/         # PrefabOverrideParser (single type, impl split across files)
│   ├── mod.rs      #   struct + new() + parse() + collect_definitions
│   │               #   + extract_string_value + tests (examples 指纹安全网)
│   ├── analysis.rs #   Section 3-7/9/14-16: 工厂调用、table.insert、return、
│   │               #   SetPrefabNameOverride、名字提取、config 表、位置工具
│   └── deep.rs     #   Section 8/10-13: 跨函数边界、带 local fn/params 的深度解析
└── types.rs        # SourceLocation, PrefabNameOverride, OverrideValue,
                    # FunctionInfo, VariableValue
```

## WHERE TO LOOK

| Task | Location | Notes |
|------|----------|-------|
| Understand the parse pipeline | `parser/mod.rs` → `parse()` / `collect_definitions()` | Two-phase: collect_definitions gathers FunctionInfo + VariableValue maps, then parse() dispatches to analysis/deep |
| Fix missed override for a factory pattern | `parser/deep.rs` → `find_prefabs_in_function_body_with_fn_tracking_and_params()` | Tracks local function defs + params across call chains. The deep-resolve function. |
| Add a new Lua call pattern | `parser/analysis.rs` → `find_*` / `analyze_prefab_call()` | Central dispatch for `Prefab()`, `table.insert()`, factory calls |
| Tweak string concatenation handling | `analysis.rs` / `deep.rs` `extract_override_*` | `OverrideValue::Static` vs `Dynamic` vs `Unknown` |
| Add an override value variant | `types.rs` → `OverrideValue` enum | Static(String), Dynamic(String), Unknown |
| Check what's publicly exported | `mod.rs` | Only parser + types re-exports |
| Verify behavior after edits | `parser/mod.rs` → `test_all_prefabs_files` | Parses all `examples/prefabs/*.lua` and asserts a stable result fingerprint |

## INTERNALS

- **Two-phase design**: `collect_definitions()` scans the AST once, populating `self.functions` / `self.variables`. Then `parse()` walks again using this index.
- **Visibility**: all split-out methods are `pub(super)` so `parser/mod.rs` can call them; struct fields stay private in `parser/mod.rs` (child modules can access ancestors' private items).
- **OverrideValue semantics**: `Static` = resolved to a known string at parse time. `Dynamic` = value depends on runtime. `Unknown` = couldn't resolve.
- **Fingerprint safety net**: `test_all_prefabs_files` hashes every example result; any behavioral change during refactors must not alter it.
- **SourceLocation**: byte-range (`start_byte`/`end_byte`) + line-range, consistent with `src/parser/lua.rs`.

## CONVENTIONS (THIS SUBDIRECTORY)

- **Stateful parser** — `PrefabOverrideParser` holds the source string, parsed AST, and accumulated maps.
- **String slices reference the source** — override values extracted via `source[*range]`.
- **Only exports `parse_prefab_overrides()` as convenience** — consumers call the free fn, not the struct directly.

## ANTI-PATTERNS (THIS SUBDIRECTORY)

- **DO NOT** add stateful helpers to a "control_flow"-style module — the old unused `control_flow.rs` was deleted; traversal lives in `parser/mod.rs`.
- **DO NOT** change distinct traversal algorithms (deep/with_params/with_iter_tables) without an explicit behavioral change + fingerprint update — they were deliberately left un-merged.
- **DO NOT** expose `FunctionInfo` or `VariableValue` publicly — they're parser-internal bookkeeping.
- **DO NOT** add regex or text-based parsing alongside the AST walker.
- **DO NOT** add new convenience fns that duplicate `parse_prefab_overrides()`.
