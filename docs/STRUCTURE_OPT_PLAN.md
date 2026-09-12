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
| P0 | 计划/进度文档建立 | ✅ | （本提交） |
| P0.1 | main.rs：dotenvy 提前到 tracing init 之前（`.env` 的 RUST_LOG 生效） | ⬜ | |
| P0.2 | `TreeDiff::to_patch` 改用 whitespace-preserving diff，whitespace-only 变更不再产出空 patch | ⬜ | |
| P0.3 | skilltree 覆盖风险：get_page 非 PageNotFound 错误不再吞掉；页面 JSON 解析失败跳过并告警（不再丢失 metainfo/icon_url 覆盖写）；`--snapshot` 传入 read_game_file/load_tuning_numbers | ⬜ | |
| P0.4 | 任务完成后失效 `skill_strings` + `diff_cache`（diff_cache 加容量上限） | ⬜ | |
| P0.5 | JobManager 资源锁（同类任务互斥 + wiki 写串行）+ 取消仅对排队中任务生效 | ⬜ | |
| P0.6 | 契约测试：JobKind.name()==serde tag、CLI 名集合==JobKind 名集合、前端 JOB_DEFS keys ⊆ serde tags；修 maintain-copyclip/corpus-sync 两处漂移 | ⬜ | |

## P0 验证方式

- `cargo test` / `cargo fmt --check` / `cargo clippy --all-targets -- -D warnings`。
- skilltree 修复用 dry-run 语义与单测验证，不做线上写。
- Web 改动以单测覆盖 JobManager 锁与取消语义（无 axum 启动的纯逻辑测试）。
