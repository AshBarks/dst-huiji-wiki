//! In-process job manager: submission, status tracking, resource locks.
//!
//! Jobs run on a dedicated OS thread with its own current-thread tokio runtime
//! (not on the axum worker pool), so CPU/IO-bound local runners cannot starve
//! the HTTP server. Output is captured by a [`CaptureReporter`] which also
//! broadcasts events to live SSE subscribers.
//!
//! Concurrency policy (P0):
//! - Same-`JobKind` jobs are mutually exclusive (two `scripts-sync` runs must
//!   never race on the same output directory).
//! - Jobs whose kind `touches_wiki()` hold a global wiki lock for their whole
//!   run, so wiki writes never interleave.
//! - Running jobs cannot be cancelled: the old "drop the future" cancel could
//!   interrupt batch uploads / file moves mid-flight. Cancellation only
//!   applies to jobs still `Queued` (waiting for their permits); the
//!   queued→running and queued→cancelled transitions are both decided under
//!   the status write lock, so they can never interleave.

use super::state::AppState;
use dst_huiji_wiki::platform::progress::{CaptureReporter, JobEvent, WriteMode};
use dst_huiji_wiki::service::{execute_job_with_mode, JobKind};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Queued,
    Running,
    Success,
    Failed,
    Cancelled,
}

impl JobStatus {
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            JobStatus::Success | JobStatus::Failed | JobStatus::Cancelled
        )
    }
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// 把 WebUI 的 `wiki_dry_run` 映射为作业的 [`WriteMode`]。
///
/// - 会写 wiki 的作业：干跑 → [`WriteMode::DryRun`]（报告 skip 原因 `dry_run`），
///   显式确认 → [`WriteMode::AutoConfirm`]；
/// - 纯本地作业不消费该字段（其 `JobKind` 自带 `dry_run`），一律 `AutoConfirm`。
///
/// 此前 WebUI 走 `WriteMode::Interactive` + `CaptureReporter::confirm` 的
/// 自动拒答，干跑在报告里表现为 `declined` 而非 `dry_run`，与 CLI 语义不一致。
pub fn write_mode_for(kind: &JobKind, wiki_dry_run: bool) -> WriteMode {
    if kind.touches_wiki() && wiki_dry_run {
        WriteMode::DryRun
    } else {
        WriteMode::AutoConfirm
    }
}

pub struct JobHandle {
    pub id: Uuid,
    pub kind: JobKind,
    /// Whether wiki writes are refused for this job (auto-"no" to any
    /// confirm). Only meaningful for wiki-touching kinds; local-only jobs
    /// gate their own disk behavior through their `JobKind` fields.
    pub wiki_dry_run: bool,
    pub status: RwLock<JobStatus>,
    pub created_at_ms: u64,
    pub started_at_ms: Mutex<Option<u64>>,
    pub finished_at_ms: Mutex<Option<u64>>,
    pub reporter: Arc<CaptureReporter>,
    pub result_json: Mutex<Option<serde_json::Value>>,
    pub error: Mutex<Option<String>>,
}

impl JobHandle {
    pub async fn status(&self) -> JobStatus {
        *self.status.read().await
    }

    /// Queued → Running 的原子转移；已取消/已结束的任务返回 `false`，不得开跑。
    async fn try_begin_run(&self) -> bool {
        let mut s = self.status.write().await;
        if *s == JobStatus::Queued {
            *s = JobStatus::Running;
            true
        } else {
            false
        }
    }

    /// Queued → Cancelled 的原子转移；已开跑/已结束的任务返回 `false`。
    pub async fn try_cancel_from_queue(&self) -> bool {
        let mut s = self.status.write().await;
        if *s == JobStatus::Queued {
            *s = JobStatus::Cancelled;
            true
        } else {
            false
        }
    }

    pub async fn summary(&self) -> serde_json::Value {
        serde_json::json!({
            "id": self.id.to_string(),
            "kind": self.kind.name(),
            "touches_wiki": self.kind.touches_wiki(),
            "wiki_dry_run": self.wiki_dry_run,
            "status": *self.status.read().await,
            "created_at_ms": self.created_at_ms,
            "started_at_ms": *self.started_at_ms.lock().await,
            "finished_at_ms": *self.finished_at_ms.lock().await,
            "event_count": self.reporter.snapshot_logs().len(),
        })
    }

    pub async fn detail(&self) -> serde_json::Value {
        let mut v = self.summary().await;
        if let Ok(obj) = serde_json::to_value(&self.kind) {
            v["params"] = obj;
        }
        v["logs"] = serde_json::to_value(self.reporter.snapshot_logs()).unwrap_or_default();
        // Structured per-page wiki diffs, in emission order.
        let diffs: Vec<serde_json::Value> = self
            .reporter
            .snapshot_logs()
            .into_iter()
            .filter_map(|ev| match ev {
                JobEvent::Diff {
                    page,
                    text,
                    added,
                    removed,
                    ..
                } => Some(serde_json::json!({
                    "page": page,
                    "text": text,
                    "added": added,
                    "removed": removed,
                })),
                _ => None,
            })
            .collect();
        v["diffs"] = serde_json::to_value(diffs).unwrap_or_default();
        if let Some(r) = self.result_json.lock().await.as_ref() {
            v["result"] = r.clone();
        }
        if let Some(e) = self.error.lock().await.as_ref() {
            v["error"] = serde_json::json!(e);
        }
        v
    }
}

const MAX_JOBS_KEPT: usize = 200;

pub struct JobManager {
    jobs: RwLock<BTreeMap<Uuid, Arc<JobHandle>>>,
    order: Mutex<VecDeque<Uuid>>,
    /// 同类任务互斥锁：按 `JobKind::name()` 懒创建、同名共享一把。
    kind_locks: Mutex<HashMap<&'static str, Arc<Mutex<()>>>>,
    /// 资源竞争域互斥锁：按 `JobSpec.resources`（"wiki"/"ktools_out"/…
    /// 懒创建共享）。不同 kind 写同一资源目录时也能互斥。
    resource_locks: Mutex<HashMap<&'static str, Arc<Mutex<()>>>>,
}

impl Default for JobManager {
    fn default() -> Self {
        Self::new()
    }
}

impl JobManager {
    pub fn new() -> Self {
        Self {
            jobs: RwLock::new(BTreeMap::new()),
            order: Mutex::new(VecDeque::new()),
            kind_locks: Mutex::new(HashMap::new()),
            resource_locks: Mutex::new(HashMap::new()),
        }
    }

    /// 同类任务的互斥锁（同名共享，不同名独立）。
    async fn kind_lock(&self, kind: &'static str) -> Arc<Mutex<()>> {
        let mut locks = self.kind_locks.lock().await;
        Arc::clone(locks.entry(kind).or_default())
    }

    /// 资源竞争域的互斥锁（同名共享）。
    async fn resource_lock(&self, resource: &'static str) -> Arc<Mutex<()>> {
        let mut locks = self.resource_locks.lock().await;
        Arc::clone(locks.entry(resource).or_default())
    }

    /// Submits a job and spawns it on the tokio runtime.
    ///
    /// `wiki_dry_run` selects the job's [`WriteMode`] via [`write_mode_for`]
    /// (`true` → `DryRun`, so only diffs are produced; `false` → `AutoConfirm`,
    /// applying writes). Local-only job kinds never consult this: their own
    /// `JobKind` fields (e.g. `ScriptsSync.dry_run`) decide how much they
    /// touch the disk.
    ///
    /// `state`（可缺省，便于测试）在任务进入终态后用于失效运行时缓存。
    pub async fn submit(
        &self,
        kind: JobKind,
        wiki_dry_run: bool,
        state: Option<Arc<AppState>>,
    ) -> Arc<JobHandle> {
        let id = Uuid::new_v4();
        let reporter = Arc::new(CaptureReporter::new(!wiki_dry_run));

        let handle = Arc::new(JobHandle {
            id,
            kind: kind.clone(),
            wiki_dry_run,
            status: RwLock::new(JobStatus::Queued),
            created_at_ms: now_ms(),
            started_at_ms: Mutex::new(None),
            finished_at_ms: Mutex::new(None),
            reporter: Arc::clone(&reporter),
            result_json: Mutex::new(None),
            error: Mutex::new(None),
        });

        // Register before spawning so the job is immediately visible.
        {
            let mut order = self.order.lock().await;
            order.push_back(handle.id);
            self.jobs
                .write()
                .await
                .insert(handle.id, Arc::clone(&handle));
            while order.len() > MAX_JOBS_KEPT {
                let evicted = match order.pop_front() {
                    Some(v) => v,
                    None => break,
                };
                let mut jobs = self.jobs.write().await;
                let removable = jobs
                    .get(&evicted)
                    .map(|j| {
                        j.status
                            .try_read()
                            .map(|s| s.is_terminal())
                            .unwrap_or(false)
                    })
                    .unwrap_or(true);
                if removable {
                    jobs.remove(&evicted);
                } else {
                    order.push_front(evicted);
                    break;
                }
            }
        }

        // Worker task: 等待资源锁（排队）→ 原子地 Queued→Running → 跑到终态。
        let kind_permit = self.kind_lock(kind.name()).await;
        let mut resource_keys: Vec<&'static str> = kind.resources().to_vec();
        resource_keys.sort_unstable();
        let mut resource_permits = Vec::with_capacity(resource_keys.len());
        for key in &resource_keys {
            resource_permits.push(self.resource_lock(key).await);
        }
        let task = Arc::clone(&handle);
        let mode = write_mode_for(&kind, wiki_dry_run);
        // 每个作业独占一条线程 + 自己的 current-thread runtime：本地重任务
        // （parser / 文件扫描 / CPU）不再占用 axum 的 worker 线程池。此前
        // 直接 `tokio::spawn` 会把整棵同步操作压在一个 worker 上。
        // runtime 在该线程内析构，避免在 async 上下文里 drop Runtime 触发
        // `Cannot drop a runtime` panic。
        std::thread::spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    tracing::error!("job {} runtime init failed: {}", task.id, e);
                    if let Ok(mut s) = task.status.try_write() {
                        *s = JobStatus::Failed;
                    }
                    if let Ok(mut err) = task.error.try_lock() {
                        *err = Some(format!("runtime init failed: {e}"));
                    }
                    if let Ok(mut f) = task.finished_at_ms.try_lock() {
                        *f = Some(now_ms());
                    }
                    task.reporter.done("failed");
                    return;
                }
            };

            rt.block_on(async move {
                let _kind = kind_permit.lock().await;
                // 严格按排序后的键序串行获取：不同任务的资源集合有交集时，
                // 全局获取顺序一致，不会形成环等待。
                let mut _resources = Vec::with_capacity(resource_permits.len());
                for permit in &resource_permits {
                    _resources.push(permit.lock().await);
                }

                // 取消与开跑的判定在同一把状态写锁下完成，二者不会交错。
                if !task.try_begin_run().await {
                    task.reporter.done("cancelled");
                    *task.finished_at_ms.lock().await = Some(now_ms());
                    if let Some(st) = state.as_ref() {
                        st.datasets.invalidate().await;
                        st.invalidate_job_caches().await;
                    }
                    return;
                }
                *task.started_at_ms.lock().await = Some(now_ms());

                match execute_job_with_mode(&task.kind, task.reporter.as_ref(), mode).await {
                    Ok(result) => {
                        *task.result_json.lock().await = Some(result);
                        *task.status.write().await = JobStatus::Success;
                        task.reporter.done("success");
                    }
                    Err(e) => {
                        tracing::error!("job {} failed: {}", task.id, e);
                        *task.error.lock().await = Some(e.to_string());
                        *task.status.write().await = JobStatus::Failed;
                        task.reporter.done("failed");
                    }
                }
                *task.finished_at_ms.lock().await = Some(now_ms());

                if let Some(st) = state.as_ref() {
                    st.datasets.invalidate().await;
                    st.invalidate_job_caches().await;
                }
            });
        });

        handle
    }

    pub async fn get(&self, id: Uuid) -> Option<Arc<JobHandle>> {
        self.jobs.read().await.get(&id).cloned()
    }

    /// Newest first.
    pub async fn list(&self) -> Vec<Arc<JobHandle>> {
        let order = self.order.lock().await;
        let jobs = self.jobs.read().await;
        order
            .iter()
            .rev()
            .filter_map(|id| jobs.get(id).cloned())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_handle(kind: JobKind) -> Arc<JobHandle> {
        Arc::new(JobHandle {
            id: Uuid::new_v4(),
            kind,
            wiki_dry_run: true,
            status: RwLock::new(JobStatus::Queued),
            created_at_ms: now_ms(),
            started_at_ms: Mutex::new(None),
            finished_at_ms: Mutex::new(None),
            reporter: Arc::new(CaptureReporter::new(false)),
            result_json: Mutex::new(None),
            error: Mutex::new(None),
        })
    }

    #[test]
    fn write_mode_maps_wiki_dry_run_and_local_jobs() {
        let wiki = JobKind::MaintainItemTable {
            output: None,
            snapshot: None,
        };
        assert_eq!(write_mode_for(&wiki, true), WriteMode::DryRun);
        assert_eq!(write_mode_for(&wiki, false), WriteMode::AutoConfirm);

        let local = JobKind::ParsePo {
            input: "x.po".into(),
            output: None,
            category: None,
        };
        assert_eq!(write_mode_for(&local, true), WriteMode::AutoConfirm);
        assert_eq!(write_mode_for(&local, false), WriteMode::AutoConfirm);
    }

    #[tokio::test]
    async fn resource_lock_shared_by_name_distinct_across_resources() {
        let mgr = JobManager::new();
        let w1 = mgr.resource_lock("wiki").await;
        let w2 = mgr.resource_lock("wiki").await;
        let k = mgr.resource_lock("ktools_out").await;
        assert!(Arc::ptr_eq(&w1, &w2));
        assert!(!Arc::ptr_eq(&w1, &k));
    }

    #[tokio::test]
    async fn kind_lock_shared_by_name_distinct_across_kinds() {
        let mgr = JobManager::new();
        let a1 = mgr.kind_lock("scripts-sync").await;
        let a2 = mgr.kind_lock("scripts-sync").await;
        let b = mgr.kind_lock("images-sync").await;
        assert!(Arc::ptr_eq(&a1, &a2));
        assert!(!Arc::ptr_eq(&a1, &b));
    }

    #[tokio::test]
    async fn running_job_cannot_be_cancelled() {
        let h = dummy_handle(JobKind::ScriptsSync {
            force: false,
            dry_run: true,
            state_path: None,
        });
        assert!(h.try_begin_run().await);
        assert_eq!(h.status().await, JobStatus::Running);
        assert!(!h.try_cancel_from_queue().await);
        assert_eq!(h.status().await, JobStatus::Running);
    }

    #[tokio::test]
    async fn queued_job_cancels_exactly_once_and_never_runs() {
        let h = dummy_handle(JobKind::ScriptsSync {
            force: false,
            dry_run: true,
            state_path: None,
        });
        assert!(h.try_cancel_from_queue().await);
        assert_eq!(h.status().await, JobStatus::Cancelled);
        assert!(!h.try_begin_run().await);
        assert!(!h.try_cancel_from_queue().await);
        assert_eq!(h.status().await, JobStatus::Cancelled);
    }

    #[tokio::test]
    async fn cancelled_then_submitted_same_status_not_reused() {
        // 终态互斥：Cancelled 之后 try_begin_run 永远失败（worker 不会复活它）。
        let h = dummy_handle(JobKind::ScriptsSync {
            force: false,
            dry_run: true,
            state_path: None,
        });
        assert!(h.try_cancel_from_queue().await);
        *h.finished_at_ms.lock().await = Some(now_ms());
        assert!(!h.try_begin_run().await);
        assert_eq!(h.status().await, JobStatus::Cancelled);
    }

    #[tokio::test]
    async fn submitted_job_reaches_terminal_state() {
        // 端到端：submit → worker 拿锁 → 执行（文件缺失 → 失败）→ 终态。
        let mgr = JobManager::new();
        let h = mgr
            .submit(
                JobKind::ParsePo {
                    input: "definitely_missing_input.po".into(),
                    output: None,
                    category: None,
                },
                true,
                None,
            )
            .await;
        for _ in 0..200 {
            if h.status().await.is_terminal() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
        assert_eq!(h.status().await, JobStatus::Failed);
        assert!(h.error.lock().await.is_some());
        assert!(h.finished_at_ms.lock().await.is_some());
    }
}
