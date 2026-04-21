# 项目重构计划

本文档记录了 `dst-huiji-wiki` 项目的代码结构优化。

## 重构状态

| 阶段 | 任务 | 状态 |
|------|------|------|
| 1 | 拆分 `models/recipe.rs` | ✅ 已完成 |
| 2 | 分离 `WikiMapper` 实现 | ✅ 已完成 |
| 3 | 移动 `tech_report.rs` | ✅ 已完成 |
| 4 | 提取 `commands` 模块 | ✅ 已完成 |
| 5 | 分离测试代码 | ⏸️ 跳过（收益较小） |

## 重构后结构

```
src/
├── main.rs              # CLI 入口（精简至 16 行）
├── lib.rs               # 库入口
├── error.rs             # 错误类型
├── context.rs           # DST 上下文
├── utils.rs             # 工具函数
│
├── commands/            # 命令处理
│   ├── mod.rs           # CLI 定义
│   └── maintain.rs      # 所有命令实现
│
├── copyclip/            # CopyClip 功能
│   ├── mod.rs           # 主逻辑 + 测试
│   └── config.rs        # 配置
│
├── mapping/             # 数据映射
│   ├── mod.rs
│   ├── builder.rs
│   ├── converter.rs     # 数据转换
│   ├── mapper.rs        # WikiMapper trait
│   ├── schema.rs        # Schema 定义
│   └── mappers/         # WikiMapper 实现
│       ├── mod.rs
│       ├── po.rs        # PoEntry 的 WikiMapper
│       └── recipe.rs    # Recipe 的 WikiMapper
│
├── models/              # 数据模型（纯数据）
│   ├── mod.rs
│   ├── po.rs            # PoEntry, PoFile
│   ├── tech_report.rs   # TechReport
│   └── recipe/
│       ├── mod.rs       # Recipe
│       ├── ingredient.rs
│       ├── options.rs
│       ├── prototyper.rs
│       └── context.rs
│
├── parser/              # 解析器
│   ├── mod.rs
│   ├── lua.rs
│   ├── po.rs
│   └── recipe.rs
│
└── wiki/                # Wiki API
    ├── mod.rs
    └── client.rs
```

## 重构详情

### 第一阶段：拆分 models/recipe.rs

**变更**：
- 创建 `models/recipe/` 目录
- 分离为 5 个文件：`mod.rs`, `ingredient.rs`, `options.rs`, `prototyper.rs`, `context.rs`
- 删除原 `models/recipe.rs`

**收益**：
- 单文件从 650+ 行减少到每个文件 20-100 行
- 职责更清晰

### 第二阶段：分离 WikiMapper 实现

**变更**：
- 创建 `mapping/mappers/` 目录
- `PoEntry` 的 `WikiMapper` 实现移到 `mapping/mappers/po.rs`
- `Recipe` 的 `WikiMapper` 实现移到 `mapping/mappers/recipe.rs`
- `models/po.rs` 和 `models/recipe/mod.rs` 只保留数据结构

**收益**：
- 数据模型与业务逻辑分离
- 更符合单一职责原则

### 第三阶段：移动 tech_report.rs

**变更**：
- `src/tech_report.rs` 移动到 `src/models/tech_report.rs`
- 更新 `lib.rs` 导出

**收益**：
- `TechReport` 归入 `models` 模块更合理
- src 根目录更整洁

### 第四阶段：提取 commands 模块

**变更**：
- 创建 `commands/` 目录
- `commands/mod.rs`: CLI 参数定义
- `commands/maintain.rs`: 所有命令处理函数
- `main.rs` 精简至 16 行

**收益**：
- main.rs 极简
- 命令处理逻辑集中管理

### 第五阶段：分离测试代码（跳过）

**原因**：
- 当前内联测试结构清晰
- 移动测试需要处理可见性问题
- 收益相对较小

**后续可考虑**：
- 当项目规模增大时再分离
- 或使用 `#[cfg(test)]` 模块保持现状

## 测试结果

所有 34 个测试通过：
```
running 34 tests
test copyclip::config::tests::test_copy_clip_config ... ok
test copyclip::config::tests::test_copy_clip_config_with_end_var ... ok
...
test result: ok. 34 passed; 0 failed; 0 ignored
```

## 注意事项

- 所有公开 API 保持向后兼容
- 每个阶段完成后运行 `cargo test` 验证
