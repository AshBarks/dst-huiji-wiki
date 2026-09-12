# knowledge 之外功能线结构优化：计划与进度

依据：`~/文档/dst-huiji-wiki-opt.md`（2026-09 审阅，事实核查于 2026-09-12 全部属实）。
本文档是执行计划与进度台账：每个小阶段完成后更新本表并 git 提交。

## 硬约束

- **全程不进行线上 wiki 写操作**；验证一律 dry-run / 单测 / 只读命令。
- 稳定 Rust、默认 fmt/clippy（CI `-D warnings`）、内联测试、无 unsafe（沿用 AGENTS.md 约定）。
- 行为等价搬迁优先于重写；先加测试与契约再动结构。

## 已拍板的决策（2026-09-12）

| 问题 | 决策 |
|---|---|
| 三处 job 契约 | 引入 **JobSpec 注册表**；P0 先补契约测试锁住漂移，P1 落地 JobSpec 本体 |
| wiki 读凭据 | **除 corpus-fetch 外读也要求登录**（匿名读可能命中旧缓存）；只做结构拆分，不改行为 |
| Web 取消语义 | **禁用运行中任务的硬取消**（不再丢弃 future）+ 资源锁；cooperative cancellation 到 P2 再评估 |
| crate 形态 | 维持单 crate 多模块；workspace 拆分仅在编译时间/边界成为实际痛点时考虑（P4） |

## 阶段计划

- **P0 数据风险小步修复**（本次会话）：skilltree 覆盖风险、TreeDiff 空白 patch、
  Web 缓存失效、Job 资源锁 + 禁硬取消、名称契约测试 + 修漂移、main.rs dotenv 顺序。
- **P1 地基**：`platform::{progress,config,fs}`；`WikiSession`（Arc 共享登录态、
  写前 ensure_login；读保持要求登录，corpus-fetch 例外）；`GameSource`；
  `WikiWriter` 统一写流程；`JobSpec` 注册表 + 参数校验；report schema 版本。
- **P2 拆分**：`service/mod.rs` 下沉各 run_* 到 feature 模块；Web 路由/DTO/缓存
  分模块；JobManager 调度器（评估 cooperative cancellation）；SSE seq 游标。
- **P3 大文件与前端**：parser/anim-index/api_data 拆分；前端 ES modules；
  技能树渲染单一源；mapping 框架按字段名对齐 + Builder 处置。
- **P4（可选）**：workspace 拆分。

## 进度台账

| 阶段 | 项 | 状态 | 提交 |
|---|---|---|---|
| 前置 | 遗留 wikitext 功能独立落库（非本计划内容，为保证每阶段提交干净） | ✅ | 886f1bf |
| P0 | 计划/进度文档建立 | ✅ | 8a60017 |
| P0.1 | main.rs：dotenvy 提前到 tracing init 之前（`.env` 的 RUST_LOG 生效） | ✅ | fa07d52 |
| P0.2 | `TreeDiff::to_patch` 改用 whitespace-preserving diff，whitespace-only 变更不再产出空 patch | ✅ | fa07d52 + c89f874 |
| P0.3 | skilltree 覆盖风险：get_page 仅 PageNotFound 走新建（fetch_failed 跳过）；页面 JSON 解析失败跳过（page_content_unparsable）；`--snapshot` 透传 | ✅ | 32e05fe |
| P0.4 | 任务终态失效 `datasets` + `skill_strings` + `diff_cache`；diff_cache 64 条上限按序淘汰 | ✅ | e45f3f7 |
| P0.5 | JobManager 资源锁（同类互斥 + wiki 写全局串行）；硬取消移除，仅排队中可取消（queued→running/cancelled 原子判定）；移除 tokio-util | ✅ | e45f3f7 |
| P0.6 | 契约测试 ×4（name 唯一且==serde tag、touches_wiki 集合、前端 JOB_DEFS ⊆ serde tags、CLI 名==JobKind 名）；修漂移 CorpusSync→CorpusFetch、maintain-copy-clip、SkilltreeWiki/Export | ✅ | a49769d |

### P0 收尾状态（2026-09-12）

- `cargo test`：626 lib + 41 bin 全绿；`cargo fmt --check` / `cargo clippy --all-targets` 干净。
- 未执行任何线上 wiki 写操作；全部验证为单测与本地构建。
- 契约测试直接抓到并修复了第三处漂移（SkillTree serde tag），证明该层有效。

### P1 补充拍板（2026-09-12 第二轮）

| 问题 | 决策 |
|---|---|
| JobSpec 落地深度 | **CLI 参数也从 JobSpec 声明生成**（clap builder 重写 commands） |
| GameSource 读优先级 | 统一 snapshot（严格）→ live → scripts.zip；dataset 获得 zip 回退（确认的行为增强） |
| report schema | **一步到位**新结构（`report_schema_version/kind/status/summary?/artifacts?/details`，旧字段收进 details） |

## 进度台账（P1）

| 阶段 | 项 | 状态 | 提交 |
|---|---|---|---|
| P1.1 | `platform::{progress,config,fs}`：Reporter/WriteMode 迁平台层（13 个底层文件改向，依赖倒置解除）；路径 env 集中解析（含 template_check 图标映射表路径统一）；原子写收编 6 处自制实现 + service 十余处裸写；scripts-sync 状态文件写前校验（修换树后写失败的缺陷） | ✅ | d3d6d55 |
| P1.2 | `platform::game_source::GameSource`：snapshot（严格）→ live → scripts.zip 唯一定义；dataset::read_game_file/list_skill_characters_local、images read_game_text 迁移；dataset 获得 zip 回退 | ✅ | 3662273 |
| P1.3 | WikiClient 登录态 `Arc<AtomicBool>` 共享（克隆同步可见，人工不变量消除）；`login(&self)` + 写前 `ensure_login`；`from_env_readonly`（corpus-fetch 唯一匿名读作业） | ✅ | 961868e |
| P1.4 | `service/wiki_write.rs::WikiWriter`：diff→确认→decide_write→edit_page（basetimestamp + span）唯一实现；迁移 5 处重复（json/copyclip/skilltree/strings/wikitext），apply_wiki_edit 删除 | ✅ | 89b0c5f |
| P1.5 | `service/job_spec.rs`：32 个 Job 的静态声明表（name/label/WikiAccess/resources/ParamSpec 参数 schema）；`JobKind::name()/touches_wiki()/resources()/label()` 从表派生；JobManager wiki 特例锁推广为通用资源锁（排序串行获取防死锁）；修 only_missing serde 默认漂移 | ✅ | 1ffa96e |
| P1.6 | CLI 从 JobSpec 生成：clap builder 重写 commands（JOB_SPECS + CLI_EXT 视角表），Commands derive 与 maintain.rs 分发器删除；契约测试锁定旗标集合==spec 参数集合；10 个解析测试 | ✅ | b601b66 |
| P1.7 | report schema v1 外壳一步到位（wrap_report 统一包壳，--report-json 同构附加元数据，app.js 适配 details）；契约测试锁定形态 | ✅ | ad3a8a7 |

### P1 收尾状态（2026-09-12）

- `cargo test`：639 lib + 21 bin 全绿；fmt/clippy 干净；全程未做线上 wiki 写。
- 契约测试现为三层：spec 表 ↔ serde tag ↔ CLI 旗标 ↔ 前端 JOB_DEFS。
- 已知留待 P2：
  - `DstContext` 仍持有缓存 ZipArchive 的读取路径（语义正确，与 GameSource 并存），service/mod.rs 拆分时统一；
  - `update::SnapshotStore`（显式双目录 diff）保持独立，不属于同一语义域；
  - upload-image/upload-icons 只复用了 `decide_write`/确认策略，multipart 上传未纳入 WikiWriter；
  - JobSpec 参数校验（非法 type/source 早失败）尚未接入执行路径，仅完成 schema 声明与测试。
