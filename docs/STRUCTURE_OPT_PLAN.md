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

## 进度台账（P2）

| 阶段 | 项 | 状态 | 提交 |
|---|---|---|---|
| P2.0 | 拆分映射盘点 | ✅ | — |
| P2.1-2.3 | service/mod.rs 拆分：`service/kind.rs`（JobKind 枚举+派生）、`update/jobs.rs`（update_scan/update_index/symbol_annotate 下沉）、`service/map_jobs.rs`、`service/maintain_jobs.rs`（ItemTable/DstRecipes/CopyClip 核心）、`service/output.rs`（WriteCtx + output helpers）；mod.rs 3390→954 行（余 ~450 行测试） | ✅ | 46494ca |
| P2.4 | web/api_data.rs → 目录模块 dataset/icons/assets/anim（mod.rs 只留 re-export 与共享 helper，路由注册不变）；1633→72+378+299+156+779 | ✅ | 2ea9be8 |
| P2.5 | SSE seq 游标：`since` 参数语义改为事件单调序号，replay/live 两侧统一 `seq > since` 过滤（修订阅-快照竞态重复）；Lagged 时发 `{"type":"resync"}`，前端重拉全量日志（修 Done 丢失流不终止） | ✅ | e70743e |
| P2.6 | JobManager 调度器评估：**取消语义维持现状**（P0 已禁硬取消+资源锁；不做协作式取消，收益/成本不划算——已拍板）。资源锁即调度器，P0 完成 | ✅ | — |
| P2.7 | AGENTS.md 结构同步 | ✅ | （本提交） |

### P2 收尾状态（2026-09-12）

- `cargo test`：639 lib + 21 bin 全绿；fmt/clippy 干净；全程未做线上 wiki 写。
- `DstContext` 与 GameSource 并存现状保留（语义均正确，重复度可控），未强行统一；
  JobSpec 参数校验仍未接入执行路径 → 列入 P3 待办。

## 进度台账（P3）

### P3 补充拍板（2026-09-12 第三轮）

| 问题 | 决策 |
|---|---|
| MappingBuilder（仅测试使用，约 500 行） | **删除** |
| 前端拆分 | **ES modules**（无打包器） |
| 技能树渲染单一源 | **方案不动**（skilltree_widget.js 与 app.js 保持双份人工同步） |
| parser 大文件重构 | **推迟**（P4/后续，动前先建差分测试） |

| 阶段 | 项 | 状态 | 提交 |
|---|---|---|---|
| P3.1 | mapping 按字段名对齐：compare_data/compare_records/merge 显式 key_field、逐字段按名对齐（历史侧缺字段视为 Null）；修 find_historical_record 误用历史下标取新值；schema 演进测试；**删除 MappingBuilder/SchemaBuilder**（537 行） | ✅ | abe98f8 |
| P3.2 | JobSpec 参数校验接入执行路径：`JobKind::validate()`（copyclip type 枚举、upload-icons source/后缀），execute_job_with_mode 入口统一调用（CLI/Web 共用） | ✅ | 6e6e8ed |
| P3.3 | anim/index.rs（2112 行）→ 目录模块：mod.rs（类型+run_index+测试）/scanner.rs（Lua 扫描器 ~1380 行） | ✅ | fa6374e |
| P3.4 | 前端 ES modules：app.js 2724 行 → 入口 64 行 + 11 个模块；`/static/js/{*file}` 白名单路由；静态资产 no-cache；JOB_DEFS 契约测试迁移到新位置；浏览器实测 10 路由全部渲染 | ✅ | 7a063f4 |
| P3.5 | 技能树单一源 / parser 重构 | ⏸ 拍板不动/推迟 | — |

### P3 收尾状态（2026-09-12）

- `cargo test`：616 lib + 21 bin 全绿；fmt/clippy 干净；全程未做线上 wiki 写。
- 前端经真实浏览器验证：10 个路由全部渲染（dashboard/jobs/recipes/translations/
  skills/constants/snapshots/anims/anim-assets 正常，icons 页为滚动加载，
  代码与拆分前逐行一致）。
- 遗留（P4/后续，均非阻塞）：
  - parser 大文件（prefab_override 3145 行单遍历器重构、skilltree.rs 拆分）——拍板推迟；
  - `DstContext` 与 GameSource 并存（语义均正确）；
  - upload 的 multipart 流程未纳入 WikiWriter（确认策略已复用）；
  - workspace 拆分（仅在编译时间/边界成为实际痛点时考虑）。

## 进度台账（P4 收尾）

P3 收尾后复核发现的非阻塞遗留，2026-09-12 拍板一并清理（parser 仅机械拆分，不做遍历器算法统一）。

| 阶段 | 项 | 状态 | 提交 |
|---|---|---|---|
| P4.0 | AGENTS 文档回补：根 `AGENTS.md` / `src/AGENTS.md` / `src/mapping/AGENTS.md` 去除已删除的 MappingBuilder、过期 commands/anim-index 结构 | ✅ | 8764f2f |
| P4.1 | 原子写补全：全库生产 `std::fs::write`（service/scripts_sync/corpus/knowledge/update）统一改 `platform::fs`（泛化 `AsRef` 签名）；`images/history.rs` 父目录 expect 改 `ensure_parent` | ✅ | 05da263 |
| P4.2 | unwrap 清理：仅修可触发点 `images/scan.rs`（非 UTF-8 stem）、`update/consts.rs`（file_name None）、`corpus/join.rs`（guarded unwrap 重写） | ✅ | a4b1f24 |
| P4.3 | Web 作业运行时隔离：`web/jobs.rs` 作业移到独立线程 + 线程内 current-thread runtime，不再占用 axum worker；`write_mode_for()` 让 wiki 干跑走 `WriteMode::DryRun`（报告 status 由 `declined` 修为 `dry_run`） | ✅ | ff776c6 |
| P4.4 | Web 边界测试：SSE `seq > since` 过滤抽纯函数并单测；补缓存失效与 JobManager 锁语义断言 | ✅ | db8029d |
| P4.5 | `DstContext` 组合 `GameSource`：统一 snapshot(严格)→live→zip 读取；删除 `archive`/`open_scripts_zip`/`read_zip_file`；`list_skill_characters` 复用 GameSource（并获得 live 目录回退） | ✅ | cc6259e |
| P4.6 | parser 机械拆分（不改算法，先建指纹安全网）：prefab_override 3145→parser/{mod,analysis,deep}；skilltree 2514→skilltree/{mod,scan,eval,cond,analyze}；删除未使用的 control_flow.rs | ✅ | （本提交） |

### P4 收尾状态（2026-09-12）

- `cargo test`：616 lib + 24 bin 全绿；`cargo fmt --check` / `cargo clippy --all-targets -D warnings` 干净。
- prefab_override 拆分前后 `test_all_prefabs_files` 指纹（`9137976922353807167`）保持不变，证明纯机械搬移。
- 全程未执行任何线上 wiki 写操作；验证为单测与本地构建。
- 顺带修正：game_source 化后 `list_skill_characters` 获得 live/zip 回退（此前 zip 模式不看 live 树）；`scripts_sync/images/meta.rs` 自制 tmp+rename 收编进 `platform::fs`。
- 明确不做：upload multipart→WikiWriter、workspace 拆分、prefab_override 遍历器算法统一。

**P4 硬约束**：仍不进行任何线上 wiki 写操作；验证为单测 / dry-run / 本地构建。

### P4 拍板（2026-09-12 第四轮）

| 问题 | 决策 |
|---|---|
| 范围 | 关闭 A–E 非结构遗留 + 统一 DstContext/GameSource + parser 机械拆分 |
| Web 作业隔离 | 独立线程 + 每线程 current-thread runtime（规避 Runtime 跨 async drop） |
| unwrap 严格度 | 只修可触发的 3 处，其余不变量安全保留 |
| parser 拆分深度 | 只做机械拆分，算法不动；upload multipart→WikiWriter 与 workspace 拆分仍不做 |

## 进度台账（P5：workspace 收编，2026-09-22）

背景：P4 收尾时拍板"workspace 拆分仅在编译时间/边界成为实际痛点时考虑"。随后发现
`dst-anim-tool = { path = "../dst-anim-tool" }` 是**仓库外 path 依赖**——fresh clone / CI
无法构建，外部项目也无法引用，必须先解决。复核结论：编译时间仍非痛点（warm check 1.4s），
因此只做"**单仓 workspace + 抽出真正自包含的模块**"，不做全量拆分、不拆多仓。

| 阶段 | 项 | 状态 | 提交 |
|---|---|---|---|
| P5.1 | dst-anim-tool 收编：git subtree 导入 `crates/dst-anim-tool`（保留 32 条历史）、补 MIT LICENSE/元数据、移除子 Cargo.lock、修 chunks_exact clippy | ✅ | fd73980 + 5302736 |
| P5.2 | Cargo workspace：根 `[workspace]`（members=crates/*, default-members=., resolver=3）+ workspace.package/dependencies，消除仓库外 path 依赖 | ✅ | 5302736 |
| P5.3 | dst-wikitext 抽为独立 crate：`src/wikitext` → `crates/dst-wikitext`（零依赖、34 tests），app 引用改向，补发布元数据/README/LICENSE | ✅ | e4e448a |
| P5.4 | CI 适配：test/clippy 改 `--workspace --exclude dst-anim-tool`（其集成测试需本机 `data/anim`）；anim-tool 单独跑 non-GUI clippy | ✅ | e4e448a |

### P5 收尾状态（2026-09-22）

- `cargo test --workspace --exclude dst-anim-tool`：646 lib + 24 bin + 34 dst-wikitext 全绿；
  `cargo fmt --all --check` / workspace clippy / anim-tool(non-GUI) clippy 干净。
- fresh clone 验证：`cargo check` 通过（原 path 依赖问题消除）；`cargo test -p dst-wikitext` 通过。
- `cargo package -p dst-wikitext --offline` 验证通过，包内容仅 src/README/LICENSE/Cargo.toml。
- 未执行任何线上 wiki 写操作；未推送到 GitHub（本地提交，待人工 push/tag）。
- 后续（按需，不阻塞）：出现真实外部消费者时再评估 dst-ktex / dst-mediawiki 抽取与 crates.io 发布；
  dst-anim-tool 集成测试保持本地跑（需 data/anim 软链），如需 CI 覆盖可引入可提交的 fixture；
  parser ↔ scripts_sync 的 `string_literal_text` 环（7 份重复实现）仍待收敛。

## 进度台账（P6：收编收尾 + edition 统一 + 断环，2026-09-23）

P5 收编后的决策落地（2026-09-23 拍板）：测试资源不打包（A+E）、edition 统一到 2021、
dst-wikitext 暂不发布、顺手收敛重复实现。

| 阶段 | 项 | 状态 | 提交 |
|---|---|---|---|
| P6.1 | 数据依赖用例治理：43 个依赖本机 DST 资源（data/anim）的用例标 `#[ignore]`；CI 纳入 anim-tool non-GUI 测试（82 个）；本地 `--include-ignored` 跑全量 125 个 | ✅ | afd6e88 |
| P6.2 | edition 统一到 2021：dst-anim-tool 由 2024 降级（`edition.workspace = true`），改写 23 处 let-chain；修 atlas.rs crop_cache 依赖 2024 if-let 临时值提前 drop 的死锁 | ✅ | ad9c1ea |
| P6.3 | 收敛 string_literal_text 7 份实现（含 1 份跨模块导入）到 parser::lua，断开 parser↔scripts_sync 唯一真环；净减 55 行 | ✅ | 10f3f5d |

### P6 收尾状态（2026-09-23）

- `cargo test --workspace --exclude dst-anim-tool`：646 lib + 24 bin + 34 dst-wikitext 全绿；
  anim-tool non-GUI：82 passed / 43 ignored（本地 `--include-ignored` 125 passed，GUI 另 3 个）；
  fmt / clippy（workspace + anim-tool non-GUI + --all-features）干净。
- 决策记录：测试资源不打包（版权）；dst-wikitext 暂不发布（外部用 git dep + tag）；
  原 dst-anim-tool 仓库由用户自行归档，monorepo 为唯一 source of truth。
- 遗留（可选）：合成 fixture（若要让数据测试进 CI）；dst-ktex / dst-mediawiki 抽取与发布（按需）。

## 进度台账（P7：抽 dst-ktex 共享 crate，2026-09-23）

P6 讨论结论：dst-ktex 去重收益高（app 与 anim-tool 各有一套 KTEX 实现），
以 app 的 ktech 兼容实现为基线合并；dst-mediawiki 暂缓（无第二消费者、竞品多）。

| 阶段 | 项 | 状态 | 提交 |
|---|---|---|---|
| P7.1 | 抽出 `crates/dst-ktex`：app 实现搬移 + 独立 `KtexError` + `trailing_pre_multiply_alpha`/`compress_bc3`；app 接入（texpresso 移入 crate、Error `#[from]` 映射、conformance 测试留 app） | ✅ | f32e55d |
| P7.2 | anim-tool 切换到 dst-ktex：566 行实现 → 60 行薄封装，specs/error 清理 KTEX 专属类型 | ✅ | f32e55d |
| P7.3 | parity 验收：3 个真实 tex 与旧解码器逐字节一致（maxΔ=0） | ✅ | f32e55d |

### P7 收尾状态（2026-09-23）

- dst-ktex 11 tests（`build_tex`/`compress_bc3` 合成 fixture，无需游戏素材）；
  app 636 lib + 24 bin（conformance 1 ignored）；anim-tool 67 passed / 43 ignored
  （本地 `--include-ignored` 110）；fmt/clippy（app workspace + anim-tool non-GUI + --all-features）全绿。
- 净减 564 行；`DECODER_VERSION` 保持 `"ktex-rs/1"`，app 图片历史无需重处理。
- 遗留（可选）：`split.rs`（atlas XML）可作为 dst-ktex 的 atlas feature 或独立 crate；
  dst-mediawiki 待第二消费者/发布意向再评估。

## 进度台账（P8：修复 CI 对 gitignore 游戏数据的编译期依赖，2026-09-23）

背景：P7 推送后 CI 首次跑到编译阶段即失败——`src/parser/prefab_override/parser/mod.rs`
的 5 个 `include_str!("examples/prefabs/*.lua")` 指向 gitignore 的本机游戏数据，
fresh clone 下 lib test 无法编译；`test_all_prefabs_files` 运行时读取也会在 CI 全 fail。
（此前 CI 被 dst-anim-tool 仓库外 path 依赖挡住，问题被掩盖。）

| 阶段 | 项 | 状态 | 提交 |
|---|---|---|---|
| P8.1 | 6 个 prefab fixture 用例：`include_str!` → 运行时 `local_prefab()` 读取 + `#[ignore]`；本地 `--include-ignored` 保持全量可跑（含指纹测试） | ✅ | 628f80e |

### P8 收尾状态（2026-09-23）

- 模拟 CI（移走 `examples/prefabs/`）：630 lib + 24 bin + 11 dst-ktex + 34 dst-wikitext 全绿，7 ignored。
- 本地有数据：prefab_override 过滤下 25 passed（含 6 个数据用例与指纹测试）。
- 原则与 anim-tool 数据用例一致：游戏数据不入库，CI 跳过、本地显式 `--include-ignored` 运行。
