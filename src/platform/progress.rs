//! 进度上报与写入策略（平台层，业务无关）。
//!
//! [`Reporter`] 是所有 job 的进度出口：CLI 用 [`StdoutReporter`]，
//! WebUI 用 [`CaptureReporter`]（事件环形缓冲 + SSE 广播）。
//! [`WriteMode`]/[`WriteDecision`]/[`decide_write`] 是 wiki 写入的统一
//! 决策函数，供所有维护流程复用。

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::broadcast;

/// How a job may write to the wiki.
///
/// - `Interactive`: ask via [`Reporter::confirm`] before every write (CLI default).
/// - `AutoConfirm`: apply writes without asking (`--yes`).
/// - `DryRun`: never write; artifacts can still be written to `--output`
///   files so the diff can be reviewed offline (`--dry-run`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WriteMode {
    #[default]
    Interactive,
    AutoConfirm,
    DryRun,
}

impl WriteMode {
    pub fn name(self) -> &'static str {
        match self {
            WriteMode::Interactive => "interactive",
            WriteMode::AutoConfirm => "auto_confirm",
            WriteMode::DryRun => "dry_run",
        }
    }
}

/// Terminal decision for one wiki-write opportunity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteDecision {
    Apply,
    /// Do not write; the payload explains why (machine-readable).
    Skip(&'static str),
}

/// Pure decision function shared by every wiki-write path.
///
/// Note that `confirmed` is only consulted in `Interactive` mode: the
/// reporter has already answered the prompt by the time this runs.
pub fn decide_write(mode: WriteMode, confirmed: bool) -> WriteDecision {
    match mode {
        WriteMode::DryRun => WriteDecision::Skip("dry_run"),
        WriteMode::AutoConfirm => WriteDecision::Apply,
        WriteMode::Interactive if confirmed => WriteDecision::Apply,
        WriteMode::Interactive => WriteDecision::Skip("declined"),
    }
}

/// How a reporter answers interactive "update wiki page?" prompts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConfirmMode {
    /// Read the answer from stdin (CLI usage).
    Interactive,
    /// Answer automatically (WebUI usage): `true` means the caller has
    /// already confirmed write operations in the UI.
    Auto(bool),
}

/// Progress sink used by all service jobs.
///
/// CLI passes [`StdoutReporter`]; the WebUI passes [`CaptureReporter`],
/// which records events for SSE streaming and later inspection.
pub trait Reporter: Send + Sync {
    /// A regular log line.
    fn log(&self, msg: String);
    /// A major phase transition (used as progress milestones).
    fn stage(&self, name: &str);
    /// A structured wiki-write diff preview for one page.
    fn diff(&self, page: &str, text: &str, added: usize, removed: usize);
    /// Ask whether a wiki write should be performed.
    fn confirm(&self, prompt: &str) -> bool;
}

pub struct StdoutReporter {
    pub confirm: ConfirmMode,
}

impl StdoutReporter {
    pub fn interactive() -> Self {
        Self {
            confirm: ConfirmMode::Interactive,
        }
    }
}

impl Default for StdoutReporter {
    fn default() -> Self {
        Self::interactive()
    }
}

impl Reporter for StdoutReporter {
    fn log(&self, msg: String) {
        println!("{}", msg);
    }

    fn stage(&self, name: &str) {
        println!("\n========== {} ==========", name);
    }

    fn diff(&self, page: &str, text: &str, added: usize, removed: usize) {
        println!("\n--- diff: {} (+{} -{}) ---", page, added, removed);
        println!("{}", text);
    }

    fn confirm(&self, prompt: &str) -> bool {
        match self.confirm {
            ConfirmMode::Auto(v) => v,
            ConfirmMode::Interactive => prompt_confirm(prompt),
        }
    }
}

fn prompt_confirm(prompt: &str) -> bool {
    use std::io::{self, BufRead, Write};

    print!("{} (y/N): ", prompt);
    if io::stdout().flush().is_err() {
        return false;
    }

    let stdin = io::stdin();
    let mut line = String::new();
    if stdin.lock().read_line(&mut line).is_err() {
        return false;
    }

    let answer = line.trim().to_lowercase();
    answer == "y" || answer == "yes"
}

/// A single captured job event, streamed to WebUI clients over SSE.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum JobEvent {
    Log {
        seq: u64,
        ts_ms: u64,
        text: String,
    },
    Stage {
        seq: u64,
        ts_ms: u64,
        name: String,
    },
    /// Structured wiki-write diff preview for one page.
    Diff {
        seq: u64,
        ts_ms: u64,
        page: String,
        text: String,
        added: usize,
        removed: usize,
    },
    /// Terminal marker pushed exactly once when the job finishes.
    Done {
        seq: u64,
        ts_ms: u64,
        status: String,
    },
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

const LOG_BUFFER_SIZE: usize = 4000;

/// Records job output into an in-memory ring buffer and broadcasts it to
/// live SSE subscribers. Confirmations are answered from a preset value so
/// that web jobs never block on stdin.
pub struct CaptureReporter {
    tx: broadcast::Sender<JobEvent>,
    logs: Mutex<VecDeque<JobEvent>>,
    seq: std::sync::atomic::AtomicU64,
    auto_confirm: bool,
}

impl CaptureReporter {
    pub fn new(auto_confirm: bool) -> Self {
        let (tx, _) = broadcast::channel(1024);
        Self {
            tx,
            logs: Mutex::new(VecDeque::with_capacity(256)),
            seq: std::sync::atomic::AtomicU64::new(0),
            auto_confirm,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<JobEvent> {
        self.tx.subscribe()
    }

    pub fn snapshot_logs(&self) -> Vec<JobEvent> {
        self.logs
            .lock()
            .expect("log mutex poisoned")
            .iter()
            .cloned()
            .collect()
    }

    fn push(&self, event: JobEvent) {
        let mut logs = self.logs.lock().expect("log mutex poisoned");
        if logs.len() >= LOG_BUFFER_SIZE {
            logs.pop_front();
        }
        logs.push_back(event.clone());
        drop(logs);
        // Sending fails when there are no subscribers; that is fine.
        let _ = self.tx.send(event);
    }

    fn next_seq(&self) -> u64 {
        self.seq.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }

    /// Pushes the terminal `Done` event for a finished job.
    pub fn done(&self, status: &str) {
        self.push(JobEvent::Done {
            seq: self.next_seq(),
            ts_ms: now_ms(),
            status: status.to_string(),
        });
    }
}

impl Reporter for CaptureReporter {
    fn log(&self, msg: String) {
        self.push(JobEvent::Log {
            seq: self.next_seq(),
            ts_ms: now_ms(),
            text: msg,
        });
    }

    fn stage(&self, name: &str) {
        self.push(JobEvent::Stage {
            seq: self.next_seq(),
            ts_ms: now_ms(),
            name: name.to_string(),
        });
    }

    fn diff(&self, page: &str, text: &str, added: usize, removed: usize) {
        self.push(JobEvent::Diff {
            seq: self.next_seq(),
            ts_ms: now_ms(),
            page: page.to_string(),
            text: text.to_string(),
            added,
            removed,
        });
    }

    fn confirm(&self, _prompt: &str) -> bool {
        self.auto_confirm
    }
}
