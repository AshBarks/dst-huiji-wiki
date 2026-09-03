# 现有维护工具质量评估报告

**评估对象**：dst-huiji-wiki（master，10,579 行 Rust / 34 个 .rs 文件）
**方法**：代码走读 + docs 对照 + 命令行为分析 + 灰机 API 实测（2026-08）
**参照**：[CODE_QUALITY_AUDIT.md](CODE_QUALITY_AUDIT.md)（2026-06-30 审计）；本报告核对其中结论的落地情况，并从"游戏更新自动同步"这一新功能的视角补充评估。
**关联**：新功能方案见 [UPDATE_IMPACT_PLAN.md](UPDATE_IMPACT_PLAN.md)。

---

## 1. 总评

| 维度 | 评分 | 说明 |
|------|------|------|
| 架构分层 | ★★★★☆ | parser / mapping / wiki / copyclip 边界清晰，依赖单向无循环；`DstContext` 聚合入口好用 |
| 解析器能力 | ★★★★☆ | full_moon AST 级解析（含 for 循环展开、跨函数变量追踪）、nom PO 解析，远超正则方案 |
| 数据正确性保障 | ★★★★☆ | P0/P1 审计修复已落地（见 §2）；converter 的 compare/merge/report 流程完备；数据带 patch 溯源 |
| wiki 客户端健壮性 | ★★☆☆☆ | 已有超时；但**无重试/节流/批量查询/编辑冲突保护**——对新功能是硬阻塞（§4.1~4.3） |
| CLI 与自动化体验 | ★★☆☆☆ | 交互确认安全，但无 `--yes`/`--dry-run` 全局化、输出非结构化、无法无人值守批处理（§4.4） |
| 测试 | ★★★☆☆ | 核心模块（mapping/models/utils）覆盖良好；commands/maintain.rs 与 mappers 仍为 0 测试 |
| 文档 | ★★★★☆ | docs 齐全且审计驱动开发闭环明显（audit → plans/*.omo → git fix commits） |

**一句话结论**：作为"单人半手动"的维护工具已达到良好水准，解析层是亮点；但要支撑"更新驱动的批量自动同步"，HTTP 层健壮性、无人值守模式与错误分类是必须先补的三块短板。

---

## 2. 上轮审计（CODE_QUALITY_AUDIT）修复情况核对

git log 显示审计发布后有一轮系统性修复，逐项核对：

| 审计问题 | 建议 | 落地情况 | 证据 |
|----------|------|----------|------|
| `full_moon::parse().ok()` 静默吞错（P0） | Result 化 | ✅ 新增 `Error::LuaParse(Vec<full_moon::Error>)` 并迁移调用点 | commit b270071 |
| `record[25]` 魔法索引（P0） | 命名字段查找 | ✅ `Schema::field_index()` | c8b4b8a / 8e7f4ec |
| `extract_ingredients().unwrap_or_default()` 等（P1） | 加 warn | ✅ 多处补 `tracing::warn!` | 5814c85 / 926823f |
| HTTP 无超时 | 加超时 | ✅ 请求/连接超时已加 | 1912cf5 |
| MappingBuilder 静默失败 | 返回 Result | ✅ `with_*` 返回 Result | 2829cf5 |
| 死代码 PoEntryMapper/RecipeMapper | 移除 | ✅ 已删 | 7512101 |
| API 过度暴露（client/variables pub） | 收敛 | ✅ `pub(crate)` + `wiki()/wiki_mut()` 代理 | d401554 |
| edit/append/prepend 90% 重复 | 抽取 do_edit | ✅ | 30944b7 |
| ingredient 映射 12 处重复 | 宏去重 | ✅ `define_ingredient_mappings!` | 7555228 |

**评价**：审计闭环质量高——问题→计划（.omo/plans）→修复提交可全程追溯。未修项：`WikiConfig.password/x_authkey` 仍为 pub 字段（低风险，单机工具可接受，但建议顺手改 getter）；`Error::WikiApi(String)` 过载依旧（在新功能场景下升级为实际问题，见 §4.3）。

> 注：上表"HTTP 无超时"行的修复对应 commit 1912cf5（feat(wiki): add HTTP request and connect timeouts）。

---

## 3. 各命令现状评估

| 命令 | 功能 | 写 wiki 条件 | 健壮性备注 |
|------|------|--------------|-----------|
| `parse-po` | PO→JSON/终端预览 | 否 | 纯本地，稳定 |
| `map-names` | NAMES 条目→ItemTable JSON schema，支持 compare/merge | 否（只产文件） | merge 语义依赖人工提供历史文件，易用性一般 |
| `map-recipes` | recipes.lua→DSTRecipes JSON | 否 | for 循环展开正确；po_file 可选补充 desc |
| `maintain-item-table` | 全量重建 ItemTable.tabx 并推送 | diff 后写 | ✅ 有变更才写；✅ `--yes`/`--dry-run`/`--report-json` 完备 |
| `maintain-dst-recipes` | 同上 + 科技树对比报告 | 同上 | 同上；TechReport 是很好的雏形 |
| `maintain-copy-clip` | 4 类常量 COPYCLIP 标记替换 | 同上 | 标记机制幂等、diff 友好，设计好 |
| `skilltree-wiki` | 提取技能树子页面 defs 并维护维基 | diff 后写 | 同维护类命令；`--output` 可同时导出 `<Char>.lua` 到本地 |
| `skilltree-export` | 提取技能树数据为本地 JSON | 否（只产文件） | 纯本地，不要求 `HUIJI__*` 凭据；每角色一个 `<角色>.json` |
| `prefab-overrides` | AST 提取 prefab 名覆盖表 | 否 | 最复杂解析器；测试占比高 |
| `scripts-sync` | 游戏更新后归档旧 scripts 树并解压新版 | 否 | 纯本地；原子 staging + 归档；快照命名供下游消费 |
| `images-sync` | 两源盘点 → 内置 KTEX 解码 → atlas 切割 → CAS 差异历史 | 否 | 纯本地；增量幂等；partial 不作 diff 基线；decoder 字段变更触发全量重处理 |

共性观察：
- **幂等性**：所有写操作都先拉历史比对，"No changes detected" 即跳过——这是正确的基线设计；
- **产物可检视**：`--dry-run` 只产出 diff/报告不写 wiki，`--report-json` 机器可读，两者已成为所有维护命令的标配；
- **溯源**：写入数据带 `"sources": "Extract data from patch <build>"`，值得保持并推广到报告。

---

## 4. 横切问题详述（按对新功能的影响排序）

### 4.1 无重试与节流（阻塞级）

`WikiClient`（wiki/client.rs）请求直发直收。实测灰机 API 存在 WAF：连续快速请求会**间歇性返回 403**（首个请求成功、紧接的同型请求被拒，间隔 15~30s 自愈），搜索接口另有偶发 `cirrussearch-backend-error`。

- 影响：任何多页面批量流程（新功能的影响扫描一次要触达几十~几百标题）必然随机失败；
- 建议：全局令牌桶节流（默认 1 QPS 可配）+ 对 403/429/5xx 指数退避重试（≤3 次，Retry-After 优先）。详见 UPDATE_IMPACT_PLAN §4.6。

### 4.2 编辑无冲突保护（高危）

`do_edit` 参数只有 action/title/token/text/summary/minor，**未携带 `basetimestamp`**。MediaWiki 在缺省时直接应用编辑、不检测中途是否有人改过该页。当前人工节奏下概率低，但自动批量写入会把窗口放大几个数量级。

- 建议：edit 流程改为 `get revision info → 带 basetimestamp 提交 → 捕获 editconflict 重读重试`；可选加 `assert=user` 防掉登录后误写。回滚支持已有基础（EditResult.newrevid 可查历史版本恢复）。

### 4.3 错误类型不足以支撑自动化分支

`Error::WikiApi(String)` 承载了"页面不存在 / token 失败 / 响应异常"等 8+ 种情况（上轮审计已指出）。在人工流程里这只是报错文案，在自动流程里程序需要区分：
- `PageNotFound` → 进 create-check 清单（正常业务分支）；
- `RateLimited` → 重试队列；
- `AuthExpired` → 重新登录后重放。

建议按此拆分变体（或至少提供 `Error::is_retryable()` / `kind()`）。

### 4.4 无人值守能力缺失

- `prompt_confirm` 直接读 stdin，批处理中会挂起或读到 EOF 退出；
- 无全局 `--yes` / `--dry-run`；dry-run 与 `-o` 落盘是两条不同代码路径；
- 输出全部是给人看的 println/diff，没有 `--report-json` 类结构化出口，CI/上层工具无法消费结果。

### 4.5 单页 API 封装不足

现有 `get_page`（单标题）与 `get_json_data`（单 tabx）。缺少：
- titles ≤50 的批量页面元信息/内容查询（MW 限制 rvlimit=1 时才能多标题）；
- allpages/continuation 封装（枚举 Data: 命名空间等场景）。
影响扫描的缓存索引（page-index）建立在这两个原语之上。

### 4.6 其他值得记录的点

- `diff_lines`（utils.rs）比较前对每行 trim：用于展示 OK；**不要**复用到"生成补丁并应用"的场景（会掩盖缩进语义，Lua 常量表里缩进虽无害但 diff 定位会漂移）。新功能的 Layer A 应基于 `similar` 直接产出 hunks，不复用 normalize 逻辑；
- `commands/maintain.rs` 743 行单体、0 测试：新功能务必走独立模块（src/update/）+ 库函数化，避免复制这个形态；
- `tracing_subscriber::fmt::init()` 已启用 env-filter feature，RUST_LOG 可控日志级别，满足批量运行的诊断需求；
- 凭据管理：`.env` 本地明文 + `.gitignore` 排除，符合单机工具惯例；`LLM__API_KEY` 沿用同一模式即可。

---

## 5. 值得保持并复用的资产

1. **COPYCLIP 标记机制**：源码定位（AST 级变量范围提取）+ 目标标记区间替换，天然幂等、diff 极小——Tier1 自动化的理想载体；
2. **全量重建 + 内容等价跳过**：tabx 管线的"No changes detected"短路是正确的幂等基线；
3. **DataDiffReport / TechReport**：对比报告的数据模型可直接喂给新功能的 report 渲染器；
4. **DstContext 统一上下文**：zip/PO/wiki client 的聚合入口让新模块接入成本很低；
5. **测试约定**：inline `#[cfg(test)]` + `.env` 缺失优雅跳过的 wiki 测试约定，新模块照做即可；
6. **文档与审计文化**：docs/ 下从想法（START）→ 规划（PLAN）→ 流程（FLOWS）→ 审计（AUDIT）链条完整，本报告与新方案延续该体系。

---

## 6. 改进建议优先级与落地状态

> 落地记录（2026-08）：P0 全部、P1/P2 主要项已完成，见状态列。另注意：并行开发已在工作区落地了
> `src/service/` 层（JobKind/Reporter/CaptureReporter）与 `src/web/` WebUI，本节 P1 的
> "库函数化" 与 "maintain.rs 拆分" 大半由该重构先行完成，剩余缺口（写入模式、报告输出）已补齐。

| 优先级 | 事项 | 服务于 | 状态 |
|--------|------|--------|------|
| **P0** | WikiClient 节流 + 403/429/5xx 退避重试 | 新功能前置（M0） | ✅ `RateLimitCfg`（默认 1 QPS/3 重试，`WIKI__QPS`/`WIKI__MAX_RETRIES` 可调）；GET 额外重试传输错误与 5xx，POST 仅重试 WAF 级 403/429；尊重 Retry-After；耗尽后返回 `RateLimited` |
| **P0** | edit 增加 basetimestamp 冲突保护 | 新功能前置 + 现有命令安全 | ✅ `edit_page` 等接受 basetimestamp；service 层从取回的 PageInfo 自动携带；同时发送 `assert=user`；`editconflict`→`Error::EditConflict`，会话失效→`AuthExpired` |
| **P0** | Error 细分（RateLimited/PageNotFound/AuthExpired） | 新功能前置 | ✅ 新增 4 变体（含 EditConflict）+ `is_retryable()`；`get_page` 缺页返回 `PageNotFound` |
| **P1** | get_pages_batch / page_exists / continuation 封装 | 影响扫描 M2 | ✅ `get_pages_meta`（≤50/批）、`page_exists`、`list_all_pages`（apcontinue 循环）；BC 布尔兼容解析有离线测试覆盖 |
| **P1** | maintain-* 库函数化 + `--yes/--dry-run/--report-json` | Tier1 调度 | ✅ service 层已有 JobKind 库接口；本次补齐 `WriteMode`（Interactive/AutoConfirm/DryRun，纯函数 `decide_write` 有测试）、CLI 三 flag（yes/dry-run 互斥由 clap 保证）、`--report-json` 机器可读报告 |
| **P1** | commands/maintain.rs 拆分与冒烟测试 | 可维护性 | ✅ 已是薄包装层；service/client/error 新增内联测试（JobKind serde 兼容旧载荷、决策表、退避上限、凭据脱敏等），全量测试通过 |
| **P2** | WikiConfig 凭据字段收敛为 getter | 安全卫生 | ✅ 字段私有化 + `host()/username()` getter + 手写脱敏 Debug（密码/authkey 不再进入任何日志或 panic 信息） |
| **P2** | 结构化日志字段规范（run_id/pair/page 维度） | 审计与排障 | ✅ main 用 uuid run_id 包裹整个命令（tracing span）；job span（job 名+write_mode）与 wiki_edit span（page 字段 + oldrevid/newrevid 结果事件）落地 |

验证：`cargo fmt --check`、`cargo clippy -D warnings`、217 个测试全部通过；
真实 API 冒烟：`maintain-copy-clip -t tech --dry-run --report-json` 在节流下全程无 403，
正确检出维基 TECH 页与当前游戏 build 的差异并以 dry_run 状态落盘报告。

P0 三项合计约 1.5~2 天，完成后即可开工 M1（见 UPDATE_IMPACT_PLAN §4.8）。
