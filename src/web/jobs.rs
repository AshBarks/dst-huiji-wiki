//! In-process job manager: submission, status tracking, cancellation.
//!
//! Jobs run as tokio tasks; output is captured by a [`CaptureReporter`]
//! which also broadcasts events to live SSE subscribers.

use dst_huiji_wiki::service::{execute_job, CaptureReporter, JobEvent, JobKind};
use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;
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
    /// Whether this job ran in dry-run mode (wiki writes skipped).
    pub dry_run: bool,
    pub status: RwLock<JobStatus>,
    pub created_at_ms: u64,
    pub started_at_ms: Mutex<Option<u64>>,
    pub finished_at_ms: Mutex<Option<u64>>,
    pub reporter: Arc<CaptureReporter>,
    pub cancel: CancellationToken,
    pub result_json: Mutex<Option<serde_json::Value>>,
    pub error: Mutex<Option<String>>,
}

impl JobHandle {
    pub async fn status(&self) -> JobStatus {
        *self.status.read().await
    }

    pub async fn summary(&self) -> serde_json::Value {
        serde_json::json!({
            "id": self.id.to_string(),
            "kind": self.kind.name(),
            "touches_wiki": self.kind.touches_wiki(),
            "dry_run": self.dry_run,
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

type DatasetInvalidator = dst_huiji_wiki::service::dataset::DatasetCache;

#[derive(Default)]
pub struct JobManager {
    jobs: RwLock<BTreeMap<Uuid, Arc<JobHandle>>>,
    order: Mutex<VecDeque<Uuid>>,
}

impl JobManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Submits a job and spawns it on the tokio runtime.
    ///
    /// `dry_run` answers wiki-write prompts: `true` skips all writes
    /// (only diffs are produced), `false` applies them directly.
    pub async fn submit(
        &self,
        kind: JobKind,
        dry_run: bool,
        datasets: Option<Arc<DatasetInvalidator>>,
    ) -> Arc<JobHandle> {
        let id = Uuid::new_v4();
        let reporter = Arc::new(CaptureReporter::new(!dry_run));

        let handle = Arc::new(JobHandle {
            id,
            kind: kind.clone(),
            dry_run,
            status: RwLock::new(JobStatus::Queued),
            created_at_ms: now_ms(),
            started_at_ms: Mutex::new(None),
            finished_at_ms: Mutex::new(None),
            reporter: Arc::clone(&reporter),
            cancel: CancellationToken::new(),
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

        // Worker task.
        let task = Arc::clone(&handle);
        tokio::spawn(async move {
            *task.status.write().await = JobStatus::Running;
            *task.started_at_ms.lock().await = Some(now_ms());

            let job_future = execute_job(&task.kind, task.reporter.as_ref());
            let token = task.cancel.clone();
            tokio::pin!(job_future);

            let outcome = tokio::select! {
                res = &mut job_future => Some(res),
                _ = token.cancelled() => None,
            };

            match outcome {
                Some(Ok(result)) => {
                    *task.result_json.lock().await = Some(result);
                    *task.status.write().await = JobStatus::Success;
                    task.reporter.done("success");
                }
                Some(Err(e)) => {
                    tracing::error!("job {} failed: {}", task.id, e);
                    *task.error.lock().await = Some(e.to_string());
                    *task.status.write().await = JobStatus::Failed;
                    task.reporter.done("failed");
                }
                None => {
                    *task.error.lock().await = Some("任务已被取消".to_string());
                    *task.status.write().await = JobStatus::Cancelled;
                    task.reporter.done("cancelled");
                }
            }
            *task.finished_at_ms.lock().await = Some(now_ms());

            if let Some(ds) = datasets {
                ds.invalidate().await;
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
