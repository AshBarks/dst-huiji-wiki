//! In-process job manager: submission, status tracking, resource locks.
//!
//! Jobs run as tokio tasks; output is captured by a [`CaptureReporter`]
//! which also broadcasts events to live SSE subscribers.
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
use dst_huiji_wiki::service::{execute_job, CaptureReporter, JobEvent, JobKind};
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
    /// wiki 写互斥：`touches_wiki` 的任务全程持有，wiki 操作互不交错。
    wiki_lock: Arc<Mutex<()>>,
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
            wiki_lock: Arc::new(Mutex::new(())),
        }
    }

    /// 同类任务的互斥锁（同名共享，不同名独立）。
    async fn kind_lock(&self, kind: &'static str) -> Arc<Mutex<()>> {
        let mut locks = self.kind_locks.lock().await;
        Arc::clone(locks.entry(kind).or_default())
    }

    /// Submits a job and spawns it on the tokio runtime.
    ///
    /// `wiki_dry_run` refuses every wiki write the job proposes (`true`
    /// skips all writes so only diffs are produced, `false` applies them
    /// directly). Local-only job kinds never consult this: their own
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
        let wiki_permit: Option<Arc<Mutex<()>>> = if kind.touches_wiki() {
            Some(Arc::clone(&self.wiki_lock))
        } else {
            None
        };
        let task = Arc::clone(&handle);
        tokio::spawn(async move {
            let _kind = kind_permit.lock().await;
            let _wiki = match wiki_permit.as_ref() {
                Some(w) => Some(w.lock().await),
                None => None,
            };

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

            match execute_job(&task.kind, task.reporter.as_ref()).await {
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
