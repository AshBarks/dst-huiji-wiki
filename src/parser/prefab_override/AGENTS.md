# PREFAB OVERRIDE PARSER

**Scope:** Extracts `PrefabNameOverride` entries from DST Lua scripts via deep full_moon AST walking. 2657-line parser.rs is the project's most complex file.

## STRUCTURE

```
prefab_override/
├── mod.rs          # 6 lines. Re-exports: PrefabOverrideParser,
│                   #   parse_prefab_overrides (free fn),
│                   #   PrefabNameOverride, OverrideValue, SourceLocation
├── parser.rs       # 2657 lines. PrefabOverrideParser struct + impl.
│                   #   One free fn: parse_prefab_overrides().
├── types.rs        # 43 lines. 5 types (2 pub, 3 pub(crate)).
└── control_flow.rs # 33 lines. Single pub fn: visit_control_flow_blocks().
```

## WHERE TO LOOK

| Task | Location | Notes |
|------|----------|-------|
| Understand the parse pipeline | `parser.rs` → `parse()` / `collect_definitions()` | Two-phase: collect_definitions gathers FunctionInfo + VariableValue maps, then parse() walks top-down |
| Fix missed override for a factory pattern | `parser.rs` → `find_prefabs_in_function_body_with_fn_tracking_and_params()` | Tracks local function defs + params across call chains. The deep-resolve function. |
| Add a new Lua call pattern | `parser.rs` → `walk_block()` / `walk_call()` | Central dispatch for `Prefab()`, `table.insert()`, factory calls |
| Tweak string concatenation handling | `parser.rs` → `resolve_override_value()` | `OverrideValue::Static` vs `Dynamic` vs `Unknown` |
| Add an override value variant | `types.rs` → `OverrideValue` enum | Static(String), Dynamic(String), Unknown |
| Tweak control flow traversal | `control_flow.rs` → `visit_control_flow_blocks()` | Called from parser.rs only. Walks if/elseif/else/while/repeat/for bodies. |
| Check what's publicly exported | `mod.rs` | Only parser + types re-exports. control_flow is pub mod but not re-exported from parent. |

## INTERNALS

- **Two-phase design**: `collect_definitions()` scans the AST once, populating `self.functions: HashMap<String, FunctionInfo>` and `self.variables: HashMap<String, VariableValue>`. Then `parse()` walks again using this index.
- **OverrideValue semantics**: `Static` = resolved to a known string at parse time. `Dynamic` = value depends on runtime (e.g., string concat with a function param). `Unknown` = couldn't resolve.
- **Function tracking**: `find_prefabs_in_function_body_with_fn_tracking_and_params()` — at 400+ lines, the single largest method. Recursively resolves anonymous function bodies passed as arguments.
- **SourceLocation**: byte-range (`start_byte`/`end_byte`) + line-range, consistent with `src/parser/lua.rs` location types.
- **Visibility**: `FunctionInfo` and `VariableValue` are `pub(crate)` — internal to the parser module only.

## CONVENTIONS (THIS SUBDIRECTORY)

- **Stateful parser** — `PrefabOverrideParser` holds the source string, parsed AST, and accumulated maps. Unlike `LuaParser`/`PoParser` which are stateless.
- **String slices reference the source** — override values extracted via `source[*range]`. The source string lives as long as the parser.
- **Only exports `parse_prefab_overrides()` as convenience** — consumers call the free fn, not the struct directly.

## ANTI-PATTERNS (THIS SUBDIRECTORY)

- **DO NOT** add stateful helpers to `control_flow.rs` — it's a pure functional walker, no struct, no state.
- **DO NOT** add new public types to `mod.rs` without checking if they're re-exported from `src/parser/` parent.
- **DO NOT** expose `FunctionInfo` or `VariableValue` publicly — they're parser-internal bookkeeping.
- **DO NOT** add regex or text-based parsing alongside the AST walker — if full_moon can't express a pattern, reconsider the approach.
- **DO NOT** add new convenience fns that duplicate `parse_prefab_overrides()` — that's the single entry point.
