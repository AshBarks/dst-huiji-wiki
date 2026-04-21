# 项目结构与依赖规划

基于 START.md 中定义的功能需求，本文档规划项目的整体架构和所需依赖库。

## 项目结构

```
dst-huiji-wiki/
├── Cargo.toml
├── src/
│   ├── main.rs                 # 程序入口
│   ├── lib.rs                  # 库入口
│   │
│   ├── parser/                 # 游戏代码解析模块
│   │   ├── mod.rs
│   │   ├── po.rs               # PO文件解析
│   │   ├── lua.rs              # Lua文件解析
│   │   ├── recipe.rs           # 配方解析
│   │   ├── skilltree.rs        # 技能树解析
│   │   └── prefab.rs           # 预制体解析
│   │
│   ├── wiki/                   # 维基维护模块
│   │   ├── mod.rs
│   │   ├── client.rs           # 维基客户端（登录、会话管理）
│   │   ├── page.rs             # 页面操作（编辑、获取）
│   │   ├── wikitext.rs         # Wikitext解析与处理
│   │   └── sync.rs             # 增量同步
│   │
│   ├── tools/                  # 维护工具模块
│   │   ├── mod.rs
│   │   ├── module_updater.rs    # 模块数据自动更新
│   │   ├── database.rs          # 数据库管理
│   │   ├── diff.rs              # 版本比较
│   │   └── search.rs            # 特征搜索
│   │
│   ├── models/                 # 数据模型
│   │   ├── mod.rs
│   │   ├── entity.rs            # 游戏实体
│   │   ├── recipe.rs            # 配方
│   │   ├── skill.rs             # 技能树
│   │   └── prefab.rs            # 预制体
│   │
│   ├── config.rs               # 配置管理
│   └── error.rs                 # 错误定义
│
├── docs/
│   ├── START.md
│   └── PLAN.md
│
└── tests/                      # 集成测试
    └── fixtures/                # 测试数据
```

## 核心依赖库

### 解析相关

| 库名 | 用途 | 版本建议 |
|------|------|----------|
| `mlua` | Lua脚本解析与执行 | 0.9+ (feature: lua54) |
| `gettext-rs` 或自定义解析 | PO文件解析 | - |
| `tree-sitter-lua` | Lua语法树解析（可选） | 0.20+ |
| `nom` | 解析器组合子（自定义解析） | 7.0+ |
| `serde` | 序列化/反序列化 | 1.0+ |
| `serde_json` | JSON处理 | 1.0+ |

### 网络与维基交互

| 库名 | 用途 | 版本建议 |
|------|------|----------|
| `reqwest` | HTTP客户端 | 0.11+ (features: json, cookies) |
| `tokio` | 异步运行时 | 1.0+ (features: full) |
| `scraper` | HTML解析 | 0.18+ |
| `url` | URL处理 | 2.0+ |

### 数据存储

| 库名 | 用途 | 版本建议 |
|------|------|----------|
| `rusqlite` | SQLite数据库 | 0.30+ |
| `sled` | 嵌入式KV存储（可选） | 0.34+ |
| `sha2` | 哈希计算（增量更新） | 0.10+ |

### 工具与辅助

| 库名 | 用途 | 版本建议 |
|------|------|----------|
| `anyhow` | 错误处理 | 1.0+ |
| `thiserror` | 自定义错误类型 | 1.0+ |
| `tracing` | 日志记录 | 0.1+ |
| `tracing-subscriber` | 日志订阅 | 0.3+ |
| `clap` | 命令行参数解析 | 4.0+ (features: derive) |
| `directories` | 配置目录管理 | 5.0+ |
| `toml` | 配置文件解析 | 0.8+ |
| `similar` 或 `diff` | 文本差异比较 | 2.0+ / 0.1+ |
| `regex` | 正则表达式 | 1.0+ |

## Cargo.toml 示例

```toml
[package]
name = "dst-huiji-wiki"
version = "0.1.0"
edition = "2021"

[dependencies]
# 异步运行时
tokio = { version = "1", features = ["full"] }

# HTTP客户端
reqwest = { version = "0.11", features = ["json", "cookies", "rustls-tls"] }

# 解析相关
mlua = { version = "0.9", features = ["lua54", "vendored"] }
nom = "7"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
regex = "1"

# HTML解析
scraper = "0.18"
url = "2"

# 数据存储
rusqlite = { version = "0.30", features = ["bundled"] }
sha2 = "0.10"

# 错误处理
anyhow = "1"
thiserror = "1"

# 日志
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# 命令行
clap = { version = "4", features = ["derive"] }

# 配置
toml = "0.8"
directories = "5"

# 差异比较
similar = "2"

[dev-dependencies]
tempfile = "3"
```

## 模块职责说明

### parser 模块
负责解析饥荒游戏代码文件：
- **po.rs**: 解析gettext格式的PO文件，提取翻译映射关系
- **lua.rs**: 解析Lua脚本文件，提取游戏数据
- **recipe.rs**: 专门处理recipes.lua，提取配方信息
- **skilltree.rs**: 解析技能树定义文件
- **prefab.rs**: 解析预制体定义，提取名称映射

### wiki 模块
负责与灰机维基交互：
- **client.rs**: 封装HTTP客户端，处理登录、Cookie管理
- **page.rs**: 页面CRUD操作
- **wikitext.rs**: Wikitext语法解析与转换
- **sync.rs**: 实现增量同步逻辑

### tools 模块
整合解析与维基功能：
- **module_updater.rs**: 自动更新维基模块数据
- **database.rs**: 本地数据存储与查询
- **diff.rs**: 版本间差异计算
- **search.rs**: 基于解析结果的特征搜索

### models 模块
定义核心数据结构，供各模块共享使用。

## 开发优先级

1. **第一阶段**: 解析模块开发
   - PO文件解析
   - Lua基础解析
   - 配方解析

2. **第二阶段**: 维基模块开发
   - 客户端与登录
   - 页面操作
   - Wikitext解析

3. **第三阶段**: 工具模块开发
   - 数据库集成
   - 自动更新工具
   - 搜索功能