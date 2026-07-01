# 代码质量审计报告

**项目**: dst-huiji-wiki
**审计日期**: 2026-06-30
**基准提交**: 08ced97 (master)
**总代码量**: 10,411 行 Rust (33 个 `.rs` 文件)
**测试**: 163 个内联测试, 全部通过
**Clippy**: 0 警告
**CI**: cargo fmt + clippy -D warnings + test + 4 目标交叉编译

---

## 总体评价

项目整体架构清晰，模块划分合理，无循环依赖，错误处理基本规范。主要问题集中在：

1. **`parser/prefab_override/parser.rs`** — 3021 行巨型文件，存在严重的代码重复和函数过长问题
2. **命令处理器** — 多职责混合，缺少关注点分离
3. **静默数据丢失** — 解析器中多处 `.ok()` / `.unwrap_or_default()` 吞掉错误
4. **API 封装不足** — 大量 `pub` 字段暴露内部实现细节

---

## 1. 代码复杂度与可维护性

### 1.1 巨型文件 (HIGH)

| 文件 | 行数 | 函数数 | >50行函数数 |
|------|------|--------|------------|
| `parser/prefab_override/parser.rs` | 3021 | 67 | 17 |
| `parser/recipe.rs` | 941 | 12 | 4 |
| `commands/maintain.rs` | 743 | 15 | 4 |
| `wiki/client.rs` | 607 | 20 | 0 |
| `mapping/converter.rs` | 579 | 16 | 1 |
| `mapping/mappers/recipe.rs` | 470 | 2 | 2 |

**parser.rs 是项目的头号技术债务**，占全项目代码的 29%，比第二大文件大 3.2 倍。

### 1.2 过长函数 (HIGH)

**parser.rs 中 >50 行的非测试函数：**

| 函数名 | 行数 | 问题 |
|--------|------|------|
| `find_override_in_stmt_deep_with_local_fns_and_params` | ~151 | 9 个 match 分支，5+ 层嵌套 |
| `find_ipairs_factory_calls_in_block` | ~127 | 与其他 3 个函数 80% 重复 |
| `find_factory_calls_in_block_with_iter_tables` | ~116 | 同上 |
| `find_prefabs_in_function_body_with_fn_tracking_and_params` | ~94 | 4 层嵌套 |
| `find_table_insert_prefabs_in_function_body_with_params` | ~93 | 同上 |
| `analyze_return_factory_call` | ~77 | 多分支 |
| `find_factory_calls_in_block` | ~75 | 8 个 match 分支 |
| `extract_prefab_name_from_arg` | ~73 | 6 个 match 分支，4 层嵌套 |
| `collect_definitions` | ~72 | 10 个 match 分支 |

**maintain.rs 中过长的函数：**

| 函数名 | 行数 | 职责数 |
|--------|------|--------|
| `maintain_crafting_names` | 92 | 7 (解析PO/构建Map/序列化/获取wiki/定位标记/替换/输出) |
| `handle_map_recipes` | 80 | 5 (解析/配置converter/转换/比较合并/输出) |
| `handle_map_names` | 69 | 4 |
| `handle_maintain_dst_recipes` | 66 | 8 (登录/解析配方/科技对比/解析PO/获取历史/转换/合并/输出) |

### 1.3 严重代码重复 (HIGH)

**4 个近似的控制流遍历函数** (合计 ~345 行)：

| 函数 | 起始行 | 用途 |
|------|--------|------|
| `find_factory_calls_in_block` | ~1702 | 遍历块收集工厂调用 |
| `find_factory_calls_in_block_with_iter_tables` | ~1787 | 同上 + iter_tables 参数 |
| `find_ipairs_factory_calls_in_block` | ~1966 | 同上 + 元组结果 |
| `find_factory_calls_in_expr` | ~2115 | 表达式变体 |

这 4 个函数共享 ~80% 的结构（递归遍历 Lua 控制流块），仅在内部检查逻辑上有差异。每个函数都复制了 If/While/Repeat/GenericFor/NumericFor/LocalFunction/FunctionDeclaration/Return 的遍历逻辑。

**其他重复：**

- `extract_factory_from_table_insert` (~420 行) 与 `extract_prefab_from_table_insert` (~457 行) 仅在内部检查条件上不同
- `output_json_result_with_update` (593-648) 与 `output_copyclip_result_with_update` (701-743) 80% 相同
- `handle_map_names` 与 `handle_map_recipes` 共享 70% 的处理流程
- `wiki/client.rs` 中 `edit_page`/`append_to_page`/`prepend_to_page` 共享 ~90% 代码（CSRF获取/参数构建/POST/结果解析）

### 1.4 深层嵌套 (MEDIUM)

| 函数 | 文件 | 嵌套深度 | 结构 |
|------|------|----------|------|
| `find_override_in_stmt_deep_with_local_fns_and_params` | parser.rs:1421 | **5+** | match/for/if let/for/if let |
| `find_ipairs_factory_calls_in_block` | parser.rs:1966 | **5** | match/if let/for/if let/for |
| `is_table_insert_with_prefab` | parser.rs:338 | **5** | if let/if/any/if/if let |
| `analyze_external_factory_call` | parser.rs:591 | **4** | for/if let/match/if let |
| `extract_prefab_name_from_arg` | parser.rs:1058 | **4** | match/if let/match/if let |

### 1.5 God Struct (MEDIUM)

**`RecipeOptions`** — 21 个 `Option<T>` 公开字段：

```
builder_tag, builder_skill, numtogive, product, placer, image,
nounlock, no_decomposition, min_spacing, testfn, action_str,
filter_text, sg_state, description, override_numtogive_fn,
is_crafting_station, icon_atlas, icon_image, hint_msg,
unlocks_from_skin, station_tag
```

这直接导致了 `mappers/recipe.rs` 中 12 个近似的 ingredient 映射（6 个 ingredient 字段 + 6 个 amount 字段，每个都是相同的 `Computed` 模式）。

---

## 2. 错误处理

### 2.1 静默数据丢失 (HIGH)

**`full_moon::parse(...).ok()` — 解析失败静默降级**

| 文件 | 行号 | 风险 |
|------|------|------|
| `parser/recipe.rs` | 686 | **严重**: 字符串替换产生无效 Lua 时，解析失败被静默吞掉，回退到原始未替换的调用。配方数据会**静默出错**且无任何错误报告。 |
| `parser/recipe.rs` | 827 | 同上 |

**`.unwrap_or_default()` — 失败时返回空值**

| 文件 | 行号 | 代码 | 风险 |
|------|------|------|------|
| `parser/recipe.rs` | 209 | `extract_ingredients(...).unwrap_or_default()` | **中等**: 配方提取失败时返回空列表，配方会以 0 个成分被创建 |
| `parser/recipe.rs` | 251 | `extract_string_expr(rhs).unwrap_or_default()` | **低**: 字符串拼接 RHS 缺失时返回空串 |
| `parser/prefab_override/parser.rs` | 123, 1096, 1166, 1256 | 同上 | **低**: 部分预制体名无警告 |

**`.unwrap_or(1)` — 数值解析失败默认为 1**

| 文件 | 行号 | 风险 |
|------|------|------|
| `parser/recipe.rs` | 328 | **中等**: 成分数量解析失败时默认为 1。如 `Ingredient("goldnugget", 40)` 解析失败会静默变为 1 |
| `parser/recipe.rs` | 531 | 同上 |

**`.ok()` — 数值解析静默返回 None**

| 文件 | 行号 | 风险 |
|------|------|------|
| `parser/recipe.rs` | 354, 454, 462, 473 | **中等**: 数值提取失败返回 None，配方被静默跳过，无警告日志 |

### 2.2 生产代码中的 unwrap/expect (LOW)

**所有 `.unwrap()` 调用均在 `#[cfg(test)]` 块内。** 生产代码中唯一的 `.expect()` 在 `context.rs:65`：

```rust
self.archive.as_mut()
    .expect("archive was verified/initialized as Some above")
```

这是合理的后置条件断言——代码刚在上方设置了 `self.archive = Some(archive)`，panic 仅在逻辑不变量被违反时触发。

### 2.3 Error 枚举分析 (MEDIUM)

**15 个变体，4 个 `#[from]` 实现：**

| 变体 | `#[from]` | 评估 |
|------|-----------|------|
| `Io(std::io::Error)` | Yes | 良好 |
| `PoParse(String)` | No | 仅 String，丢失 nom 结构化位置信息 |
| `InvalidPoEntry(String)` | No | 同上 |
| `EnvVarNotFound(String)` | No | 可接受 |
| `ParseError(String)` | No | **过度重载**：同时用于 Lua 解析失败、字段未找到、排序违规 |
| `Http(reqwest::Error)` | Yes | 良好 |
| `WikiApi(String)` | No | **过度重载**：8+ 种不同条件共用一个变体 |
| `LoginFailed(String)` | No | 可接受 |
| `EditFailed(String)` | No | 可接受 |
| `Config(String)` | No | 可接受 |
| `Zip(ZipError)` | Yes | 良好 |
| `ArchiveFileNotFound(String)` | No | 可接受 |
| `DstDirNotFound(String)` | No | 可接受 |
| `InvalidPath(String)` | No | 可接受 |
| `Json(serde_json::Error)` | Yes | 良好 |

**缺失的 `From` 实现：**

- `full_moon::Error` → 应添加 `LuaParse` 变体（当前用 `ParseError(format!("..."))` 手动转换）
- `std::env::VarError` → 当前 4+ 处手动 `map_err`，可简化

### 2.4 HTTP 客户端健壮性 (MEDIUM)

| 问题 | 位置 | 风险 |
|------|------|------|
| **无超时配置** | `wiki/client.rs:142` | 默认无超时，挂起的 API 调用会永久阻塞 |
| **无重试逻辑** | 全局 | 瞬态故障（DNS/5xx/429）导致命令立即失败 |
| 硬编码主机 | `client.rs:7` | `DEFAULT_WIKI_HOST = "dontstarve.huijiwiki.com"` |
| 魔法数字 | `client.rs:143` | `redirect::Policy::limited(10)` 无解释 |

---

## 3. API 设计与封装

### 3.1 过度暴露的 pub 字段 (HIGH)

**20 个结构体所有字段均为 `pub`，无任何私有字段：**

| 结构体 | pub 字段数 | 问题 |
|--------|-----------|------|
| `RecipeOptions` | 21 | 厨房水槽结构体 |
| `RecipeContext` | 7 | `variables` 是解析器内部实现细节，不应公开 |
| `PageInfo` | 6 | API 响应类型，可接受 |
| `EditResult` | 6 | 同上 |
| `Recipe` | 6 | 数据模型，缺少不变量保证 |
| `WikiConfig` | 4 | 包含密码等敏感信息 |
| `PoEntry` | 4 | 数据模型 |
| `FieldSchema` | 5 | 配置类型 |
| `WikiJsonData` | 5 | 序列化类型 |
| `VariableLocation` | 5 | 查询结果类型 |
| `VariableRange` | 5 | 同上 |
| `FieldLocation` | 4 | 同上 |
| `SourceLocation` | 4 | 同上 |
| `PrefabNameOverride` | 3 | 同上 |
| `DataDiffReport` | 5 | 报告类型 |
| `CopyClipConfig` | 4 | 配置类型 |
| `CopyClipResult` | 4 | 结果类型 |
| `Ingredient` | 4 | 数据模型 |
| `TechReport` | 3 | 报告类型 |
| `FieldChange` | 3 | 抮告类型 |

**关键问题：**

- `WikiConfig` 的 `password` 和 `x_authkey` 字段为 `pub`，任何持有引用的代码都可读取凭据
- `RecipeContext::variables` 是 Lua 变量跟踪的内部实现，不应是公共 API
- `DstContext::client` 为 `pub`，导致 10 处直接调用 `ctx.client.*`

### 3.2 死代码 (HIGH)

**`PoEntryMapper` 和 `RecipeMapper`** — 定义并重新导出但从未使用：

```rust
// mapping/mappers/po.rs
pub struct PoEntryMapper;  // 从未实例化

// mapping/mappers/recipe.rs
pub struct RecipeMapper;   // 从未实例化

// WikiMapper 实际实现：
impl WikiMapper for PoEntry { ... }   // 在模型类型上
impl WikiMapper for Recipe { ... }    // 在模型类型上
```

这两个结构体通过 `lib.rs` 重新导出到公共 API，但从未被引用、实例化或使用。

### 3.3 MappingBuilder 静默失败 (HIGH)

4 个 `with_*` 方法在目标字段未找到时静默返回 `self`：

```rust
pub fn with_overwrite(mut self, target: &str) -> Self {
    if let Some(rule) = self.rules.iter_mut().find(|r| r.target_field == target) {
        rule.merge_strategy = MergeStrategy::Overwrite;
    }
    self  // 未找到时静默返回！
}
```

`with_preserve_history`、`with_merge_priority`、`with_custom_merge` 同样存在此问题。目标字段名拼写错误会被静默吞掉。

### 3.4 MappingBuilder::build() 返回裸元组 (MEDIUM)

```rust
pub fn build(self) -> (Schema, Vec<FieldMappingRule<T>>, String)
```

三个不相关类型作为元组返回，调用者必须按位置解构。重排元素会静默破坏所有调用者。

### 3.5 MergeStrategy::Custom 的 PartialEq 不可靠 (MEDIUM)

```rust
(MergeStrategy::Custom(a), MergeStrategy::Custom(b)) => std::ptr::fn_addr_eq(*a, *b),
```

函数指针地址比较在跨编译单元、动态链接或 LLVM 去重时不可靠。两个语义相同的闭包可能比较为不等。

### 3.6 DstContext 职责过多 (MEDIUM)

DstContext 承担 5 项职责：

1. 环境变量读取 (`from_env()`)
2. ZIP 归档管理 (`open_scripts_zip()`, `read_zip_file()`)
3. 维基客户端创建和存储 (`client: WikiClient`)
4. PO 文件解析 (`parse_po_file()`)
5. 元数据生成 (`sources()`)

修改解析器 API 或维基客户端 API 都会强制重新编译 `context.rs`。

### 3.7 MappingBuilder 与 SchemaBuilder DRY 违规 (LOW)

两者都定义了 `.with_title()`、`.required()`、`.with_default()`，逻辑完全相同。

---

## 4. 测试覆盖

### 4.1 测试分布

| 模块 | 总行数 | 测试数 | 测试代码行 | 覆盖评估 |
|------|--------|--------|-----------|----------|
| `mapping/builder.rs` | 499 | 24 | ~246 | 良好 |
| `mapping/schema.rs` | 386 | 15 | ~203 | 良好 |
| `mapping/converter.rs` | 579 | 17 | ~187 | 良好 |
| `mapping/mapper.rs` | 378 | 10 | ~165 | 良好 |
| `models/po.rs` | 216 | 14 | ~144 | 良好 |
| `parser/prefab_override/parser.rs` | 3021 | 15 | ~484 | 不足 (仅 16% 代码有测试) |
| `models/recipe/context.rs` | 222 | 12 | ~115 | 良好 |
| `copyclip/mod.rs` | 278 | 6 | ~113 | 良好 |
| `parser/lua.rs` | 420 | 8 | ~123 | 中等 |
| `utils.rs` | 112 | 11 | ~91 | 良好 |
| `parser/po.rs` | 264 | 6 | ~83 | 良好 |
| `parser/recipe.rs` | 941 | 4 | ~71 | **不足** (仅 7% 代码有测试) |
| `wiki/client.rs` | 607 | 2 | ~79 | **不足** (依赖 .env，CI 中跳过) |
| `commands/maintain.rs` | 743 | 0 | 0 | **无测试** |
| `mapping/mappers/recipe.rs` | 470 | 0 | 0 | **无测试** |
| `mapping/mappers/po.rs` | 114 | 0 | 0 | **无测试** |
| `context.rs` | 88 | 0 | 0 | **无测试** |
| `error.rs` | 52 | 0 | 0 | 可接受 (仅类型定义) |

### 4.2 关键测试缺口

- **`commands/maintain.rs`** (743 行) — 0 个测试。所有命令处理器无单元测试。
- **`mapping/mappers/recipe.rs`** (470 行) — 0 个测试。最复杂的映射器无测试。
- **`mapping/mappers/po.rs`** (114 行) — 0 个测试。
- **`parser/recipe.rs`** (941 行) — 仅 4 个测试，覆盖率极低。
- **`wiki/client.rs`** — 2 个测试均依赖 `.env` 配置，CI 中跳过。

---

## 5. 文档

### 5.1 公共 API 文档缺失

**约 40+ 个公共项缺少 `///` 文档注释**，包括：

- 所有 `WikiDataConverter` 的公共方法
- 所有 `PoLookupTable` 的公共方法
- `FieldSchema` 及其构建器方法
- `WikiMapper` trait 的所有方法
- `DstContext` 的所有方法
- `diff_lines` 函数
- `Error` 枚举

### 5.2 缺少的 trait 实现

| 类型 | 缺少 | 影响 |
|------|------|------|
| `FieldMappingRule<T>` | `Debug` | 仅有 `Clone`，无法调试打印 |
| `PageInfo`, `EditResult`, `WikiConfig` | `Serialize/Deserialize` | 公共 API 类型无法序列化 |
| 多数模型类型 | `Display` | 无用户友好的格式化输出 |

---

## 6. 模块依赖

### 6.1 依赖图 (无循环)

```
lib.rs
  +-- context    -> models, parser, wiki, error
  +-- copyclip   -> parser (lua), error
  +-- error      -> (无依赖)
  +-- mapping    -> models, error
  |   +-- builder   -> mapper, schema
  |   +-- converter -> mapper, schema, models::PoEntry
  |   +-- mapper    -> schema
  |   +-- schema    -> (serde only)
  |   +-- mappers   -> mapping, models::Recipe/PoEntry
  +-- models     -> (serde only)
  +-- parser     -> models, error
  |   +-- lua       -> error
  |   +-- po        -> models::PoEntry/PoFile, error
  |   +-- recipe    -> models (all 5 types), error
  |   +-- prefab_override -> error
  +-- utils      -> similar crate
  +-- wiki       -> error

main.rs (binary only)
  +-- commands   -> everything (expected, integration layer)
```

**结论：依赖方向正确，无循环。** `commands` 是二进制专用模块，导入所有库模块是预期行为。

### 6.2 特性嫉妒 (MEDIUM)

`commands/maintain.rs` 中的 `output_json_result_with_update` 和 `output_copyclip_result_with_update` 函数处理完整的维基更新流程（diff/提示/编辑），这些逻辑应属于 `wiki/` 模块。

---

## 7. 其他发现

### 7.1 魔法索引 (MEDIUM)

`converter.rs:127` 使用 `record[25]` 硬编码索引访问描述字段。如果 schema 字段顺序变化，此索引会静默指向错误字段。

### 7.2 Lua 字符串替换脆弱性 (LOW)

`parser/recipe.rs:670-698` 的 `substitute_var_in_call_with_locals` 方法将 AST 序列化为字符串后进行文本替换再重新解析。如果变量名是另一个标识符的子串（如用 `GOLD` 替换时误匹配 `GOLDNUGGET`），会产生误报。

### 7.3 diff_lines 忽略空白差异 (LOW)

`utils.rs` 的 `normalize_lines` 在比较前 trim 所有行，可能隐藏有意义的缩进差异。

### 7.4 mapping/mod.rs 异常的模块声明顺序 (LOW)

`mod converter;` 出现在 `pub use` 语句之后，而非与其他 `mod` 声明一起。虽然编译正确，但对读者造成困惑。

---

## 8. 优先修复建议

### P0 — 立即修复 (数据正确性风险)

| # | 问题 | 位置 | 建议 |
|---|------|------|------|
| 1 | `full_moon::parse().ok()` 静默吞掉解析错误 | `recipe.rs:686,827` | 替换为 `map_err` + `tracing::warn!`，或返回 Result |
| 2 | `extract_ingredients().unwrap_or_default()` 静默创建空配方 | `recipe.rs:209` | 至少添加 `tracing::warn!`，考虑返回 Result |
| 3 | `record[25]` 魔法索引 | `converter.rs:127` | 使用命名字段查找替代位置索引 |

### P1 — 短期修复 (可维护性)

| # | 问题 | 位置 | 建议 |
|---|------|------|------|
| 4 | 4 个近似控制流遍历函数 | `parser.rs` | 提取通用 `walk_blocks` 闭包参数化遍历 |
| 5 | 12 个近似 ingredient 映射 | `mappers/recipe.rs` | 使用宏或索引查找消除重复 |
| 6 | `edit_page`/`append`/`prepend` 重复 | `wiki/client.rs` | 提取共享的 `do_edit` 方法 |
| 7 | 删除死代码 PoEntryMapper/RecipeMapper | `mappers/` | 删除结构体及重新导出 |
| 8 | MappingBuilder 静默失败 | `builder.rs` | `with_*` 方法返回 `Result<Self>` 或添加 build 时验证 |
| 9 | 无 HTTP 超时 | `client.rs:142` | 添加 `.timeout(Duration::from_secs(30))` |

### P2 — 中期改进 (架构)

| # | 问题 | 位置 | 建议 |
|---|------|------|------|
| 10 | DstContext 职责过多 | `context.rs` | 拆分为 ScriptsArchive + WikiSession |
| 11 | `client` 字段泄漏 | `context.rs:13` | 改为 `pub(crate)` + 代理方法 |
| 12 | 命令处理器多职责 | `maintain.rs` | 提取 pipeline 逻辑到 wiki/ 模块 |
| 13 | `build()` 返回裸元组 | `builder.rs:147` | 创建 `MappingDefinition<T>` 命名结构体 |
| 14 | `ParseError` 过度重载 | `error.rs` | 添加 `LuaParse` 专用变体 |
| 15 | `WikiApi` 过度重载 | `error.rs` | 拆分为 `TokenError`/`PageNotFound`/`ContentMissing` 等 |
| 16 | `RecipeContext::variables` 公开 | `context.rs` | 改为 `pub(crate)` |

### P3 — 长期改进 (质量)

| # | 问题 | 位置 | 建议 |
|---|------|------|------|
| 17 | parser.rs 3021 行 | `parser.rs` | 拆分为 collector/resolver/walker 子模块 |
| 18 | RecipeOptions 21 字段 | `options.rs` | 拆分为逻辑子结构体 |
| 19 | 命令处理器无测试 | `maintain.rs` | 添加集成测试 |
| 20 | 映射器无测试 | `mappers/` | 添加单元测试 |
| 21 | 公共 API 缺文档 | 全局 | 添加 `///` 文档注释 |
| 22 | `MergeStrategy::Custom` PartialEq | `mapper.rs:27` | 添加 `id: &'static str` 字段用于比较 |

---

## 9. 指标汇总

| 指标 | 值 | 评估 |
|------|-----|------|
| 总代码行 | 10,411 | - |
| 最大文件行 | 3,021 (parser.rs) | 过大 |
| 最大函数行 | ~151 | 过大 |
| 测试总数 | 163 | 偏少 |
| 测试覆盖率 (有测试的模块) | 20/33 文件 | 61% |
| 生产代码 unwrap | 0 | 优秀 |
| 生产代码 expect | 1 (合理) | 优秀 |
| 循环依赖 | 0 | 优秀 |
| Clippy 警告 | 0 | 优秀 |
| 公共结构体全 pub 字段 | 20 | 过多 |
| 死代码公共类型 | 2 | 需清理 |
| TODO/FIXME | 0 | - |
| unsafe 块 | 0 | 优秀 |
