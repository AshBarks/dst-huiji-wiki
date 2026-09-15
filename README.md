# DST 灰机维基维护工具

[![CI](https://github.com/AshBarks/dst-huiji-wiki/actions/workflows/ci.yml/badge.svg)](https://github.com/AshBarks/dst-huiji-wiki/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-2021%20edition-orange.svg)](https://www.rust-lang.org/)
[![GitHub release](https://img.shields.io/github/v/release/AshBarks/dst-huiji-wiki?include_prereleases)](https://github.com/AshBarks/dst-huiji-wiki/releases)

用于维护饥荒联机版（Don't Starve Together）灰机维基的 Rust 工具集。

## 功能

- **游戏数据解析**: 解析配方（recipes.lua）、PO 翻译文件、Lua 脚本（含预制体重定向解析与技能树提取）
- **游戏资源同步**: 游戏更新后同步 scripts 树为快照（`scripts-sync`），并处理图片资源（`images-sync`：两源盘点 → 内置 KTEX 解码 → atlas 切割 → 差异历史）
- **数据映射**: 将游戏数据映射为维基 JSON schema，支持对比与合并历史数据
- **维基客户端**: 与灰机维基 API 交互，支持登录、页面编辑等操作（节流 + 重试）
- **CopyClip 维护**: 基于标记替换更新维基 Lua 模块（科技常量、制作分类等）
- **语料抓取**: 抓取维基主命名空间全量语料到本地（`corpus-fetch`），并重建派生索引（prefab 注册表等）
- **快照差异与影响评估**: 对比两个游戏脚本快照，产出 impact.json / changes.patch（`update-scan`）
- **代码关联索引**: 构建代码→页面关联索引（`update-index`），支撑 Page→Symbol 标注（`symbol-annotate`）
- **知识库管线**: 基于 LLM 生成符号知识文档（SymbolDoc）、页面映射骨架与同步（`knowledge-*`），并给出页面覆盖缺口建议（`page-assist`）
- **WebUI**: 网页控制台提交任务、实时日志、数据浏览（见下文）

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
| `KTOOLS__OUT_DIR` | 可选，图片管线产物根目录（`current/` 工作集 + `history/` 差异历史，缺省 `./output/ktools`） | `/mnt/data/ktool_output` |
| `ANIM__OUT_DIR` | 可选，动画历史管线产物根目录（`history/` 差异历史，缺省 `./output/anim`） | `/mnt/data/anim_history` |
| `WIKI__QPS` | 可选，维基 API 每秒请求数上限（默认 1） | `1` |
| `WIKI__MAX_RETRIES` | 可选，403/429/5xx 退避重试次数（默认 3） | `3` |
| `LLM__API_KEY` | 可选，LLM API 密钥（未配置时 LLM 相关命令跳过模型调用） | `sk-...` |
| `LLM__BASE_URL` | 可选，LLM API 地址（默认 `https://api.openai.com/v1`） | `https://api.openai.com/v1` |
| `LLM__MODEL` | 可选，LLM 模型名（默认 `gpt-4o-mini`） | `gpt-4o-mini` |
| `LLM__TIMEOUT_SECS` | 可选，单次请求超时秒数（默认 120） | `300` |

#### 获取灰机维基认证信息

1. 注册并登录 [灰机维基](https://huijiwiki.com/)
2. 在个人设置中获取 API 密钥或使用账号密码登录

#### DST 游戏目录位置

- **Linux**: `~/.steam/debian-installation/steamapps/common/Don't Starve Together`
- **macOS**: `~/Library/Application Support/Steam/steamapps/common/Don't Starve Together`
- **Windows**: `C:\Program Files (x86)\Steam\steamapps\common\Don't Starve Together`

## WebUI（网页控制台）

内置 WebUI，可在浏览器中提交维护任务、实时查看日志、浏览与可视化游戏数据：

```bash
cargo run --release -- serve            # 默认 http://127.0.0.1:8420
cargo run --release -- serve --host 0.0.0.0 --port 9000   # 自定义监听
```

功能一览：

| 页面 | 说明 |
|------|------|
| 概览 | 数据规模卡片 + 最近任务 |
| 任务 | 提交所有 CLI 命令（含**干跑模式**）、任务列表、SSE 实时日志流、取消 |
| 配方/材料 | 配方检索（按科技/名称）、点击材料即可反查引用它的配方 |
| 烹饪模拟 | 选择 4 个食材与锅类型，按最高优先级等概率随机并展示全部候选；支持食材搜索与快照数据 |
| 翻译 | PO 分类进度条形图、85k+ 条目分页检索 |
| 技能树 | 直接使用游戏内 `pos/connects` 坐标 1:1 还原角色技能树（含中文标题） |
| 常量 | TUNING 表 5000+ 常量搜索浏览 |
| 快照对比 | 任选两个 `scripts_日期` 快照对比新增/移除/修改的配方与翻译 |
| 动画对比 | 任选两个 anim-sync 历史版本对比 zip/dyn 文件级与结构化 diff |

安全说明：默认只绑定 `127.0.0.1`；涉及维基写入的任务在页面中默认勾选"干跑模式"，
取消勾选并确认后才会真实编辑页面。

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

#### `prefab-overrides` - 解析预制体重定向

从 Lua 文件中提取预制体名称重定向（工厂模式、控制流等）。

```bash
cargo run --release -- prefab-overrides [OPTIONS] --input <FILE>
```

| 参数 | 简写 | 说明 |
|------|------|------|
| `--input <FILE>` | `-i` | 输入的 Lua 文件路径（必需） |
| `--output <FILE>` | `-o` | 输出的 JSON 文件路径（可选） |

---

#### `maintain-item-table` - 维护物品表

从 DST 游戏文件提取名称数据并更新到维基。

```bash
cargo run --release -- maintain-item-table [OPTIONS]
```

| 参数 | 简写 | 说明 |
|------|------|------|
| `--output <FILE>` | `-o` | 输出的 JSON 文件路径（可选） |
| `--yes` | | 跳过确认，直接写入维基（无人值守） |
| `--dry-run` | | 只生成产物与 diff，不写入维基（与 `--yes` 互斥） |
| `--report-json <FILE>` | | 将机器可读的执行报告写入该文件 |

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

同样支持 `--yes` / `--dry-run` / `--report-json <FILE>` 参数。

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

除上述参数外，还支持：

| 参数 | 说明 |
|------|------|
| `--yes` | 跳过确认，直接写入维基（无人值守） |
| `--dry-run` | 只生成产物与 diff，不写入维基（与 `--yes` 互斥） |
| `--report-json <FILE>` | 将机器可读的执行报告写入该文件 |

**示例：**

```bash
# 更新所有类型
cargo run --release -- maintain-copy-clip

# 只更新科技等级常量
cargo run --release -- maintain-copy-clip -t tech

# 只更新配方构建器标签查找表
cargo run --release -- maintain-copy-clip -t rbtl

# 试运行：只看 diff 不写维基，并把结果报告落盘
cargo run --release -- maintain-copy-clip -t filters --dry-run --report-json report.json
```

---

#### `skilltree-wiki` - 维护技能树子页面

从 DST 游戏文件提取角色技能树，组装为 `模块:Skilltree/<Char>` 子页面的
`defs` JSON 并维护维基页面（保留页内 `metainfo` 与 `icon_url`）。

```bash
cargo run --release -- skilltree-wiki [OPTIONS]
```

| 参数 | 简写 | 说明 |
|------|------|------|
| `--character <STRING>` | `-c` | 只处理名字包含该子串的角色（如 walter） |
| `--output <DIR>` | `-o` | 同时把每个子页面内容写到该目录（`<Char>.lua`） |
| `--snapshot <NAME>` | | 使用指定 scripts 快照（默认当前 scripts 树） |
| `--yes` | | 跳过确认，直接写入维基（无人值守） |
| `--dry-run` | | 只生成产物与 diff，不写入维基（与 `--yes` 互斥） |
| `--report-json <FILE>` | | 将机器可读的执行报告写入该文件 |

**说明：** 此命令需要配置 `DST__ROOT`。`--output` 用于把每个角色的子页面内容
同时导出到本地，便于离线检视或后续手工处理。

**示例：**

```bash
# 导出全部角色技能树子页面到本地目录（同时按交互确认是否写维基）
cargo run --release -- skilltree-wiki -o output/skilltree

# 只处理 walter，并只做 dry-run 查看 diff
cargo run --release -- skilltree-wiki -c walter --dry-run -o output/skilltree
```

---

#### `skilltree-export` - 导出技能树数据到本地

纯本地导出技能树解析结果（含中文标题/描述、坐标、连接、锁与标签等），
不访问维基，也不要求 `HUIJI__*` 凭据。

```bash
cargo run --release -- skilltree-export [OPTIONS]
```

| 参数 | 简写 | 说明 |
|------|------|------|
| `--character <STRING>` | `-c` | 只处理名字包含该子串的角色（如 walter） |
| `--output <DIR>` | `-o` | 输出目录（默认 `output/skilltree`），每个角色写入 `<角色>.json` |
| `--snapshot <NAME>` | | 使用指定 scripts 快照（默认当前 scripts 树） |
| `--report-json <FILE>` | | 将机器可读的执行报告写入该文件 |

**示例：**

```bash
# 导出全部角色技能树 JSON 到 output/skilltree
cargo run --release -- skilltree-export

# 导出 walter 到指定目录
cargo run --release -- skilltree-export -c walter -o output/skilltree
```

---

#### `scripts-sync` - 同步游戏 scripts 树

游戏更新后归档旧 scripts 树为 `scripts_<时间戳>` 快照，解压新的 `scripts.zip` 并记录版本（纯本地操作，`update-scan`/`knowledge-*` 依赖快照命名）。

```bash
cargo run --release -- scripts-sync [OPTIONS]
```

| 参数 | 说明 |
|------|------|
| `--force` | 忽略版本一致，强制同步 |
| `--dry-run` | 只报告将要执行的动作，不修改任何文件 |
| `--state <FILE>` | 版本状态文件路径（默认 `./dst_version.txt`） |
| `--report-json <FILE>` | 将机器可读的执行报告写入该文件 |

---

#### `images-sync` - 处理游戏图片资源

处理 `data/databundles/images.zip` 与 `data/images/` 两源（散装目录中的遗留 png 仅计数、绝不读取/写入）的图片资源：解压 → 内置 KTEX 解码（DXT1/3/5/RGB，无需外部 ktools）→ 按 atlas XML 切割为独立 sprite，最终产物进入内容寻址差异历史（纯本地操作）。

```bash
cargo run --release -- images-sync [OPTIONS]
```

| 参数 | 说明 |
|------|------|
| `--force` | 忽略增量与幂等检查，全量重跑 |
| `--dry-run` | 只盘点并报告计划，不写任何文件 |
| `--report-json <FILE>` | 将机器可读的执行报告写入该文件 |

**产物布局**（`KTOOLS__OUT_DIR`，缺省 `./output/ktools`）：

```text
current/{unzipped,split,decoded}/   最新 build 工作集（下游消费入口）
history/objects/<h[:2]>/<h>.png     内容寻址对象仓，仅最终产物，跨版本去重
history/manifests/<build>.json      每 build 全量清单 + 相邻 diff + 输入 hash
```

**增量与历史**：输入 hash 不变且产物在盘则跳过；每次运行记录相对上一完整版本的 added/removed/changed（差异细化到单个 sprite）；`--force` 或解码器版本变更时全量重处理。

---

#### `anim-sync` - 动画历史同步

游戏更新完成后运行，扫描当前 `data/anim`（含 `dynamic/*.dyn`），把原始动画包归档到内容寻址历史，并生成相对上一版本的 diff。

```bash
cargo run --release -- anim-sync [OPTIONS]
```

| 参数 | 说明 |
|------|------|
| `--force` | 忽略幂等检查，强制重新扫描/归档 |
| `--dry-run` | 只盘点并报告计划，不写任何文件 |
| `--label <LABEL>` | 版本标签，默认取 `DST__ROOT/version.txt` |
| `--out <DIR>` | 历史根目录，默认 `$ANIM__OUT_DIR` 或 `./output/anim` |
| `--report-json <FILE>` | 将机器可读的执行报告写入该文件 |

**产物布局**（`ANIM__OUT_DIR`，缺省 `./output/anim`）：

```text
history/objects/<h[:2]>/<sha256>.zip|.dyn   内容寻址原始动画包
history/manifests/<label>.json              每版本全量清单 + 相邻 diff
```

---

#### `anim-diff` - 动画目录结构化对比

对比两个动画目录中的 `.zip` / `.dyn`，输出文件级和解析后的结构化 diff。

```bash
cargo run --release -- anim-diff <OLD_DIR> <NEW_DIR> [OPTIONS]
```

| 参数 | 说明 |
|------|------|
| `--zip <REL>` | 只对比指定相对路径，如 `dynamic/abigail_ice.dyn` |
| `--report-json <FILE>` | 将机器可读的执行报告写入该文件 |

---

#### `anim-index` - 构建 prefab 与动画文件关联索引

扫描 `scripts/prefabs` 中的 `Asset("ANIM", ...)` / `Asset("DYNAMIC_ANIM", ...)`，建立 `prefab 变体 -> 动画文件` 的静态关联，并与 `data/anim` 对账。支持展开常见工厂函数（`MakeAxe`、`makeassets`、`makeassetlist` 等），并对命中的动画包解析 `anim.bin` / `build.bin`，建立 `prefab -> bank/animation` 内容级索引。纯本地操作，不做皮肤相关 `.dyn` / `PKGREF` 关联。

```bash
cargo run --release -- anim-index <SCRIPTS_ROOT> [OPTIONS]
```

| 参数 | 说明 |
|------|------|
| `--anim <DIR>` | 动画资源目录；缺省由 scripts 路径推导为 `../data/anim` |
| `--out <FILE>` | 输出 JSON 文件路径，默认 `output/anim-index.json` |
| `--report-json <FILE>` | 将机器可读的执行报告写入该文件 |

**产物**：`output/anim-index.json`，内容包含：

- `prefabs`：每个 prefab 变体关联的动画文件、相关 build/package 文件，以及从 `anim.bin` / `build.bin` 聚合出的 `banks` / `animations` / `builds` / `symbols` / `atlases`；
- `anim_files`：每个动画文件的反向引用列表与内容摘要；
- `unresolved`：动态/无法静态解析的 Asset 引用，带文件与行号；
- `stats`：文件数、引用数、唯一动画数、缺失数等摘要。

WebUI 提供“动画素材”页面，可按 prefab 检索并预览/导出 GIF 或 PNG 序列。

---

#### `corpus-fetch` - 抓取维基语料

抓取维基主命名空间全量语料到本地 `wikis/<host>/` 目录（gitignore，不入仓库）。

```bash
cargo run --release -- corpus-fetch [OPTIONS]
```

| 参数 | 说明 |
|------|------|
| `--full` | 忽略增量对账，全量重抓所有页面 |
| `--dir <DIR>` | 语料根目录（默认 `wikis`） |
| `--dry-run` | 只枚举与对账出报告，不写任何本地文件 |
| `--rc` | recentchanges 增量通道（检查点缺失/过期自动回落枚举对账） |

---

#### `update-index` - 构建代码关联索引

扫描游戏脚本，构建代码→页面关联索引（基础设施 A），缓存到 `output/atlas/<build>/`。

```bash
cargo run --release -- update-index <ROOT> [OPTIONS]
```

| 参数 | 说明 |
|------|------|
| `<ROOT>` | 游戏脚本根目录（当前树或快照目录） |
| `--out <DIR>` | 输出目录（默认 `output/atlas/<build 号>/`） |

---

#### `update-scan` - 快照差异与影响评估（只读）

对比两个游戏脚本快照，产出 `impact.json` 与 `changes.patch`。

```bash
cargo run --release -- update-scan <OLD> <NEW> [OPTIONS]
```

| 参数 | 说明 |
|------|------|
| `<OLD>` | 旧快照（时间戳或目录名） |
| `<NEW>` | 新快照（时间戳、目录名，或 `current` 表示当前 scripts 树） |
| `--out <DIR>` | 输出目录（默认 `output/scan/<old>_<new>/`） |
| `--corpus <DIR>` | 语料 host 根目录（`wikis/<host>/`），提供时附加 Layer B 定级摘要 |
| `--annotate <FILE>` | 输出 fn 标注骨架（prefabs 前 50 文件 + hound.lua） |

---

#### `corpus-index` - 重建语料派生索引

从本地语料树重建派生索引（prefab 注册表、区域、facts 等），纯本地操作。

```bash
cargo run --release -- corpus-index [OPTIONS]
```

| 参数 | 说明 |
|------|------|
| `--dir <DIR>` | 语料根目录（默认 `wikis`，其下需恰好一个 host 树） |
| `--join <FILE>` | 代码侧 index.json 路径，额外产出 join_report.json 校准报告 |
| `--dry-run` | 只构建并报告统计，不写工件 |

---

#### `symbol-annotate` - Page→Symbol 标注

生成高引用 symbol 证据包和 Prompt；可选读取已有 LLM 输出并生成跨页一致性报告（纯本地，不写 wiki）。

```bash
cargo run --release -- symbol-annotate <ROOT> --corpus <DIR> [OPTIONS]
```

| 参数 | 说明 |
|------|------|
| `<ROOT>` | 游戏脚本根目录（当前树或快照目录） |
| `--corpus <DIR>` | 语料 host 根目录（`wikis/<host>/`） |
| `--limit <N>` | 只处理引用量最高的前 N 个 symbol（默认 20） |
| `--verdicts <FILE>` | 可选的 LLM/人工标注结果文件（JSON 数组或 SymbolAnnotationResponse） |
| `--llm` | 配置了 `LLM__API_KEY` 时直接调用大模型生成标注；未配置则跳过 |
| `--batch-pages <N>` | LLM 分批大小（默认 40，0 = 不按页数设限） |
| `--batch-max-chars <N>` | 每批渲染输入的字节预算（默认 32000） |
| `--skip-no-fact-pages` | 零候选证据的页面不送 LLM，本地合成 low-confidence missing 判定 |
| `--out <DIR>` | 输出目录（默认 `output/symbol-annotate/`） |

---

#### `knowledge-scan-symbols` - 符号知识文档扫描（M1）

LLM 阅读符号源码，产出/更新 SymbolDoc 知识文档（`knowledge/` 目录）。

```bash
cargo run --release -- knowledge-scan-symbols <ROOT> [OPTIONS]
```

| 参数 | 说明 |
|------|------|
| `<ROOT>` | 游戏脚本根目录（当前树或快照目录） |
| `--category <STRING>` | 符号类别：`component` / `brain` / `behaviour`（默认 `component`） |
| `--knowledge-dir <DIR>` | 知识文档根目录（默认 `knowledge`） |
| `--corpus <DIR>` | 语料 host 根目录，提供则启用 pass2 语料归因（link-wiki） |
| `--sample-pages <N>` | pass2 每符号采样的页面数（默认 8） |
| `--limit <N>` | 只处理引用量最高的前 N 个符号（默认 20） |
| `--concurrency <N>` | 并行处理的组件数（默认 1 = 串行） |
| `--force` | 忽略 sha/prompt_rev 一致性，强制重扫 |
| `--refresh-auto` | 不调 LLM，仅用 AutoInfobox 冷数据刷新现有文档的 auto_maintained |
| `--confirm-empty` | pass2 二次确认：采样 ≥3 页但判空时追加一次复查 |
| `--pass2-names <LIST>` | 仅对指定文件名词干跑 pass2（逗号分隔） |
| `--pick-names <LIST>` | 仅选取指定文件名词干的符号（逗号分隔） |

---

#### `knowledge-scan-wiki` - 页面映射骨架（M2a）

构建 PageSymbolMap 确定性骨架（路由/反转/数值配对，不调 LLM）。

```bash
cargo run --release -- knowledge-scan-wiki <ROOT> --corpus <DIR> [OPTIONS]
```

| 参数 | 说明 |
|------|------|
| `<ROOT>` | 游戏脚本根目录（当前树或快照目录） |
| `--corpus <DIR>` | 语料 host 根目录（必需） |
| `--knowledge-dir <DIR>` | 知识文档根目录（默认 `knowledge`） |
| `--audit` | M2b：对确定性零证据对跑 LLM 审计（收编 symbol-annotate verdict） |
| `--audit-symbols <LIST>` | 审计符号词干（逗号分隔），默认按缺口取前 10 |
| `--audit-max-pages <N>` | 每符号送审页数上限（默认 60） |
| `--audit-batch-pages <N>` | LLM 每批页数上限（默认 20） |
| `--audit-batch-max-chars <N>` | LLM 每批字符数上限（默认 24000） |
| `--report` | M2c：不重建地图，聚合现有 knowledge/pages 出报表（summary.json） |
| `--classify` | M2 收尾：语义不一致对三分类（确定性） |

---

#### `knowledge-sync` - 知识同步（M3）

代码变更 → 脏 SymbolDoc → 页面锚点交叉（确定性）。

```bash
cargo run --release -- knowledge-sync [OLD] [NEW] [OPTIONS]
```

| 参数 | 说明 |
|------|------|
| `<OLD>` | 旧快照（时间戳或目录名；`--review` 复核模式可省略） |
| `<NEW>` | 新快照（默认 `current` 表示当前 scripts 树） |
| `--knowledge-dir <DIR>` | 知识文档根目录（默认 `knowledge`） |
| `--rescan` | 级联重扫脏文档（调 LLM；默认仅输出清单） |
| `--limit <N>` | 详列的脏文档数上限（默认 20） |
| `--corpus <DIR>` | 语料 host 根目录，提供则启用 prefab→页面交叉 |
| `--draft` | Tier2：起草页面修订建议（需 `--corpus` 与 LLM 配置） |
| `--review <FILE>` | Tier2 复核：裁决文件路径（对既有报告的建议逐条 approve/reject） |

---

#### `page-assist` - 页面覆盖缺口建议

给定页面输出覆盖缺口建议清单（读 PageSymbolMap，不调 LLM）。

```bash
cargo run --release -- page-assist [OPTIONS]
```

| 参数 | 说明 |
|------|------|
| `--page <ID|TITLE>` | 页面 id（纯数字）或标题（精确匹配） |
| `--all` | 全库缺口榜（忽略 `--page`） |
| `--json` | 输出 JSON 而非 Markdown |
| `--knowledge-dir <DIR>` | 知识文档根目录（默认 `knowledge`） |
| `--attribute` | 编辑归因模式（区域 × SymbolDoc，需 `--corpus`） |
| `--corpus <DIR>` | 语料根目录（归因模式必需） |

---

#### `serve` - 启动 WebUI

见上文 [WebUI（网页控制台）](#webui网页控制台)。

## 项目结构

```
src/
├── main.rs               # 二进制入口（clap 分发）
├── lib.rs                # 库根（8 个公共模块 + 关键类型再导出）
├── commands/             # CLI 参数定义（Commands 枚举）+ 全部命令处理器
├── service/              # 任务执行引擎（JobKind、WriteMode、进度、快照对比）
├── web/                  # WebUI 服务器（axum，仅二进制模块）
├── parser/               # 游戏代码解析模块
│   ├── lua.rs            # Lua 变量/字段定位
│   ├── po.rs             # PO 文件解析（nom）
│   ├── recipe.rs         # 配方解析（full_moon AST）
│   ├── skilltree.rs      # 技能树提取（pos/connects 坐标 + 常量折叠）
│   └── prefab_override/  # 预制体重定向解析
├── models/               # 数据模型（Recipe、PoEntry、TechReport）
├── mapping/              # 数据→维基映射框架（WikiMapper 特征 + MappingBuilder）
├── wiki/                 # MediaWiki API 客户端
├── copyclip/             # 维基模块内容更新（标记替换）
├── corpus/               # 语料抓取、派生索引（prefab 注册表/区域/facts）
├── knowledge/            # 知识库管线（SymbolDoc 扫描、页面映射、同步）
├── scripts_sync/         # 游戏资源同步
│   ├── mod.rs            # scripts.zip 同步（版本检测→快照归档→解压）
│   └── images/           # images-sync 图片管线（两源扫描/内置 KTEX 解码/切割/差异历史）
├── update/               # 快照差异、影响评估、代码关联索引（atlas）
├── context.rs            # DstContext（zip 归档、维基客户端、环境变量）
├── error.rs              # 错误枚举 + Result<T>
├── llm.rs                # LLM 客户端（标注/知识文档生成）
└── utils.rs              # diff_lines 工具
```

## 许可证

[MIT License](LICENSE)