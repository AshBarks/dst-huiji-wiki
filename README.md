# DST 灰机维基维护工具

[![CI](https://github.com/AshBarks/dst-huiji-wiki/actions/workflows/ci.yml/badge.svg)](https://github.com/AshBarks/dst-huiji-wiki/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-2021%20edition-orange.svg)](https://www.rust-lang.org/)
[![GitHub release](https://img.shields.io/github/v/release/AshBarks/dst-huiji-wiki?include_prereleases)](https://github.com/AshBarks/dst-huiji-wiki/releases)

用于维护饥荒联机版（Don't Starve Together）灰机维基的 Rust 工具集。

## 功能

- **配方解析**: 解析游戏中的配方数据（recipes.lua）
- **PO 文件解析**: 解析 gettext 格式的翻译文件
- **Lua 解析**: 解析 Lua 脚本文件，提取游戏数据
- **维基客户端**: 与灰机维基 API 交互，支持登录、页面编辑等操作
- **数据映射**: 将游戏数据映射为维基所需的格式

## 依赖

- [Rust](https://www.rust-lang.org/tools/install) 2021 Edition (推荐使用 rustup 安装)
- Lua 5.2（用于 full_moon 解析）

### 安装 Rust

```bash
# macOS/Linux
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 或访问官方安装指南
# https://www.rust-lang.org/tools/install
```

## 安装

```bash
git clone https://github.com/AshBarks/dst-huiji-wiki.git
cd dst-huiji-wiki
cargo build --release
```

## 配置

复制 `.env.example` 为 `.env` 并填写配置：

```bash
cp .env.example .env
```

### 环境变量说明

| 变量名 | 说明 | 示例 |
|--------|------|------|
| `HUIJI__USERNAME` | 灰机维基用户名 | `your-username` |
| `HUIJI__PASSWORD` | 灰机维基密码 | `your-password` |
| `HUIJI__X_AUTHKEY` | 灰机维基站点认证密钥 | `site-authkey` |
| `DST__ROOT` | DST 游戏根目录路径 | `/path/to/Don't Starve Together` |

#### 获取灰机维基认证信息

1. 注册并登录 [灰机维基](https://huijiwiki.com/)
2. 在个人设置中获取 API 密钥或使用账号密码登录

#### DST 游戏目录位置

- **Linux**: `~/.steam/debian-installation/steamapps/common/Don't Starve Together`
- **macOS**: `~/Library/Application Support/Steam/steamapps/common/Don't Starve Together`
- **Windows**: `C:\Program Files (x86)\Steam\steamapps\common\Don't Starve Together`

## 使用

```bash
# 运行程序
cargo run --release -- <command>

# 查看帮助
cargo run --release -- --help
```

### 命令详解

#### `parse-po` - 解析 PO 文件

解析 gettext 格式的翻译文件，提取翻译条目。

```bash
cargo run --release -- parse-po [OPTIONS] --input <FILE>
```

| 参数 | 简写 | 说明 |
|------|------|------|
| `--input <FILE>` | `-i` | 输入的 PO 文件路径（必需） |
| `--output <FILE>` | `-o` | 输出的 JSON 文件路径（可选，不指定则打印到终端） |
| `--category <STRING>` | `-c` | 按类别过滤条目（可选） |

**示例：**

```bash
# 解析 PO 文件并输出到 JSON
cargo run --release -- parse-po -i chinese_s.po -o output.json

# 只提取 NAMES 类别的条目
cargo run --release -- parse-po -i chinese_s.po -c NAMES
```

---

#### `map-names` - 映射名称数据

将 PO 文件中的名称条目映射为维基数据格式。

```bash
cargo run --release -- map-names [OPTIONS] --input <FILE>
```

| 参数 | 简写 | 说明 |
|------|------|------|
| `--input <FILE>` | `-i` | 输入的 PO 文件路径（必需） |
| `--output <FILE>` | `-o` | 输出的 JSON 文件路径（可选） |
| `--compare <FILE>` | | 与历史数据对比的 JSON 文件路径（可选） |
| `--merge` | | 与历史数据合并（需要 `--compare`） |
| `--version <STRING>` | `-v` | 数据版本号（可选） |

**示例：**

```bash
# 生成名称映射数据
cargo run --release -- map-names -i chinese_s.po -o names.json -v "1.0.0"

# 与历史数据对比
cargo run --release -- map-names -i chinese_s.po --compare old.json

# 与历史数据合并
cargo run --release -- map-names -i chinese_s.po --compare old.json --merge -o merged.json
```

---

#### `map-recipes` - 映射配方数据

将 Lua 配方文件映射为维基数据格式。

```bash
cargo run --release -- map-recipes [OPTIONS] --input <FILE>
```

| 参数 | 简写 | 说明 |
|------|------|------|
| `--input <FILE>` | `-i` | 输入的 Lua 文件路径（必需） |
| `--output <FILE>` | `-o` | 输出的 JSON 文件路径（可选） |
| `--compare <FILE>` | | 与历史数据对比的 JSON 文件路径（可选） |
| `--merge` | | 与历史数据合并（需要 `--compare`） |
| `--po-file <FILE>` | | 用于描述查找的 PO 文件路径（可选） |
| `--version <STRING>` | `-v` | 数据版本号（可选） |

**示例：**

```bash
# 解析配方文件
cargo run --release -- map-recipes -i recipes.lua -o recipes.json

# 使用 PO 文件补充描述信息
cargo run --release -- map-recipes -i recipes.lua --po-file chinese_s.po -o recipes.json
```

---

#### `maintain-item-table` - 维护物品表

从 DST 游戏文件提取名称数据并更新到维基。

```bash
cargo run --release -- maintain-item-table [OPTIONS]
```

| 参数 | 简写 | 说明 |
|------|------|------|
| `--output <FILE>` | `-o` | 输出的 JSON 文件路径（可选） |

**说明：** 此命令需要配置 `DST__ROOT` 环境变量，会自动从游戏文件中提取数据并与维基历史数据合并。

**示例：**

```bash
# 维护物品表数据
cargo run --release -- maintain-item-table

# 输出到本地文件
cargo run --release -- maintain-item-table -o item_table.json
```

---

#### `maintain-dst-recipes` - 维护配方表

从 DST 游戏文件提取配方数据并更新到维基。

```bash
cargo run --release -- maintain-dst-recipes [OPTIONS]
```

| 参数 | 简写 | 说明 |
|------|------|------|
| `--output <FILE>` | `-o` | 输出的 JSON 文件路径（可选） |

**说明：** 此命令需要配置 `DST__ROOT` 环境变量，会自动从游戏文件中提取配方数据并与维基历史数据合并，同时生成科技树对比报告。

**示例：**

```bash
cargo run --release -- maintain-dst-recipes
```

---

#### `maintain-copy-clip` - 维基模块数据更新

从 DST 游戏文件提取常量数据并更新到维基模块。

```bash
cargo run --release -- maintain-copy-clip [OPTIONS]
```

| 参数 | 简写 | 说明 |
|------|------|------|
| `--type <STRING>` | `-t` | 更新类型（可选，不指定则运行所有类型） |
| `--output <FILE>` | `-o` | 输出文件路径（可选） |

**支持的类型：**

| 类型值 | 别名 | 说明 | 更新的维基页面 |
|--------|------|------|----------------|
| `recipe_builder_tag_lookup` | `rbtl` | 配方建造者标签查找表 | `模块:Constants/RecipeBuilderTagLookup` |
| `tech` | - | 科技等级常量 | `模块:Constants/Tech` |
| `crafting_filters` | `filters` | 制作分类 | `模块:Constants/CraftingFilters` |
| `crafting_names` | `names` | 制作名称翻译 | `模块:Constants/CraftingNames` |

**示例：**

```bash
# 更新所有类型
cargo run --release -- maintain-copy-clip

# 只更新科技等级常量
cargo run --release -- maintain-copy-clip -t tech

# 只更新配方构建器标签查找表
cargo run --release -- maintain-copy-clip -t rbtl
```

## 项目结构

```
src/
├── parser/          # 游戏代码解析模块
│   ├── lua.rs       # Lua 文件解析
│   ├── po.rs        # PO 文件解析
│   └── recipe.rs    # 配方解析
├── wiki/            # 维基维护模块
│   └── client.rs    # 维基客户端
├── models/          # 数据模型
├── mapping/         # 数据映射
├── copyclip/        # CopyClip 功能
└── commands/        # 命令行接口
```

## 许可证

[MIT License](LICENSE)
