# 技能树图标管理方案（SKILLTREE_ICONS_PLAN）

**状态**：实施中（进度见文末 checklist）
**定稿决策**：并入 `ICON_SOURCES` 体系 / 节点内联 `title` 提取 / 例外表兜底 / 孤儿与 wiki 遗留仅报告

## 1. 背景与调查结论

制作栏/物品栏图标已有完整管理管线（images-sync 元数据 → 五态 → upload-icons →
WebUI 页面）。技能树图标（`split/skilltree_icons/`，atlas 321 张）目前只有切图与
CAS 历史，缺命名、上传状态管理与浏览页。调查确认：

### 1.1 已存在的基础

| 环节 | 现状 |
|------|------|
| 本地切图 | images-sync 已切 `split/skilltree_icons/` 321 张，manifest/CAS 历史齐全 |
| 游戏侧语义 | `prefabs/skilltree_<角色>.lua` 节点带 `icon` 字段（解析器已提取，非锁节点回退为节点名）；节点体内联 `title = STRINGS.SKILLTREE.<角色>.<键>_TITLE` |
| PO 翻译 | `SKILLTREE` 命名空间 626 个 `_TITLE` 键，中英翻译齐全 |
| wiki 端 | 分类:技能树图标 345 个文件，命名 = 本地 stem 的 MediaWiki 归一化（`walter_ammo_bag.png` → `File:Walter ammo bag.png`）；角色页经 `{{Skill|<stem>}}` 模板引用 |
| 上传工具 | `upload-image` CLI 支持该目录自动分类描述（无批量/状态管理） |

### 1.2 对账数字（2026-09 定稿前调查）

- **icon → 节点 → TITLE 链**可自动命名约 79%（名称约定法）；改用**内联 `title` 引用**后接近全量，
  实际剩余缺口见 §3.2 例外表。
- 255/255 抽样命名全部有中文翻译。
- **30 个本地有、wiki 无**（从未上传）→ 五态「未上传」直接暴露。
- **54 个 wiki 有、本地无**（历史版本遗留，如 Walter 旧弹弓弹药系列）→ 仅报告。
- **13 个 atlas 孤儿**（无静态节点引用：`willow_refuel`、`wendy_petal_1` 等，
  多为运行时动态换图/开发遗留）→ 仅报告。
- wiki 交互渲染件 `零件:Skilltree.js` 图标路径 `./images/skilltree_icons/...` 实测 404
  （本站上传在 huijistatic 哈希路径）→ Phase 3 修复。

### 1.3 与物品栏/制作栏管线的本质差异

1. **命名链不同**：不能查 `STRINGS.NAMES.<stem>`（物品栏）或 Lua 定义表（制作栏），
   而是「解析器节点表 icon → (角色, 节点) → 节点内联 `title` 键 → PO `SKILLTREE` 命名空间」。
2. **wiki 标题规则不同且更简单**：wiki 名 = 文件 stem 归一化，与游戏英文名无关
   （游戏名 "Ammo Hoarder"，wiki 文件叫 "Walter ammo bag"）。自动标题 = `normalize(stem)`，
   不需要 PO 即与 wiki 现状 100% 对齐；PO 名仅作展示（`name_en`/`name_zh`）。
3. **分组方式**：按角色分节（图标名前缀 = 角色名，无下划线，约 17 节）。
4. **数量与频率**：321 个、低频变动——采用制作栏页的仪表盘形态。

## 2. 定稿决策

| # | 决策点 | 定稿 |
|---|--------|------|
| 1 | 架构 | 并入 `ICON_SOURCES` 第三来源（`skilltree`），五态/元数据/上传作业/WebUI 组件全部复用 |
| 2 | 显示名来源 | 解析器提取节点**内联 `title` 引用**（`SkillNode.title_key`），不用节点名约定 |
| 3 | 无 TITLE 图标 | 例外表 `config/skilltree_icon_names.json` 兜底（文件名 → {en, zh}） |
| 4 | 孤儿图标 & wiki 历史遗留 | 仅报告，不做清理动作 |

## 3. 分期实施

### Phase 1：命名链 + 来源注册（后端）

- `parser/skilltree`：`SkillDef`/`SkillNode` 新增 `title_key`（节点体内联
  `title = STRINGS.SKILLTREE.<角色>.<键>_TITLE` 的点路径，取末段键）。
- `meta.rs`：
  - `NameMaps` 之外新增 `SkilltreeNames`（icon stem → (角色, TITLE 键)，来自解析器节点表；
    (角色, 键) → (en, zh)，来自 PO；例外表合并）；`read_skilltree_names(dst_root)`
    经 GameSource 读 PO + 全部 `skilltree_<char>.lua`（角色清单来自 PO 键）。
  - `CraftingGroup` 泛化为 `IconGroup`：`Crafting { section, key }` / `Skilltree { character }`，
    字段 `crafting` → `group`（schema 随 images-sync 重建，旧字段被忽略即可）。
  - `effective_title` 新增 `skilltree` 分支：标题 = `auto_title(文件 stem)`。
  - `build_meta` 接入 skilltree 命名与分组。
- `icons.rs`：`ICON_SOURCES` 注册 `{ id: "skilltree", dir: "skilltree_icons", label: "技能树图标" }`。
- `upload_icons.rs`：`icon_category`/`upload_comment` 加 skilltree 分支
  （`[[分类:技能树图标]]`）。
- 指纹测试扩展：真实数据下技能树图标命名抽样 + 分组断言。

### Phase 2：例外表 + 分类对账

- 例外表 `config/skilltree_icon_names.json`（`{ "文件名.png": { "en": ..., "zh": ... } }`）：
  内联提取后仍无 TITLE 的图标逐条核对填充（游戏无字符串的编辑名注明）；
  加载入口 `SKILLTREE__ICON_NAMES` 环境变量可覆盖路径。
- `WikiClient::list_category_members`：列出分类成员（continuation 感知）。
- `refresh_icon_meta`（`check_wiki` 开启时）：对 skilltree 来源做一次
  分类:技能树图标 对账——「本地有 wiki 无」（= missing，走上传）与
  「wiki 有本地无」（历史遗留）计数 + 样本写入报告，仅报告不动作。

### Phase 3：WebUI 页面

- `#/skilltree-icons`：制作栏页仪表盘形态——一次拉全（`source=skilltree&page_size=500`）、
  按 `group.character` 分节（角色中文名硬编码花名册，EN 兜底）、游戏内名为主卡面、
  中文拼音排序、摘要行（全部已上传/缺失计数）、新图标徽章、无搜索无分页；
  批量上传缺失复用 `upload-icons --source skilltree`。
- 详情弹窗/映射编辑复用 `icon_shared.js`，零改动。
- `crafting_icons.js` 适配 `group` 字段改名。

### Phase 4：渲染件图标路径修复

- `skilltree_widget.js`（wiki 用 `Skilltree.js` 的源）：技能树内容图标
  （`skilltree_icons`/`global_redux`）在 MediaWiki 运行时（`window.mw` 存在）改走
  `mw.util.getUrl("Special:FilePath/<归一化名>")`；本地预览路径保留。
- UI 铬件（`skilltree` atlas 的 frame/selected 等）wiki 端文件覆盖情况未核，
  本次不动，注释说明（若未来部署交互渲染件再补）。
- `skilltree-wiki --output` 本地重发验证；wiki 端上传为人工步骤。

## 4. 验证

- 单元：解析器 `title_key`、`SkilltreeNames` 解析链、`effective_title` skilltree 分支、
  `IconGroup` serde、`build_meta` skilltree 条目（fixture）。
- 指纹（env 守护）：真实数据命名抽样（`walter_ammo_bag` → 弹药囤积者 / Ammo Hoarder，
  标题 `Walter ammo bag.png`）、分组覆盖、命名率。
- 端到端：images-sync 元数据刷新（named/missing 计数）、`upload-icons --source
  skilltree --dry-run`（预期 30 missing）、WebUI 双页目视。

## 5. 进度

- [x] 调查与定稿（本文档）
- [x] 既有改动分批提交（d5d3406 制作栏命名管线 / a89b6d7 页面拆分）
- [ ] Phase 1：命名链 + 来源注册（parser title_key、SkilltreeNames、IconGroup 泛化、
      ICON_SOURCES/upload 接入、指纹测试）
- [ ] Phase 2：例外表 + 分类对账
- [ ] Phase 3：WebUI 技能树图标页
- [ ] Phase 4：渲染件图标路径修复
- [ ] wiki 端人工步骤：上传更新后的 零件:Skilltree.js（Phase 4 产物）
