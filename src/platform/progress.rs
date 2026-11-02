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

/// Severity of a [`JobEvent::Log`] line. Drives WebUI colouring/filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            LogLevel::Debug => "debug",
            LogLevel::Info => "info",
            LogLevel::Warn => "warn",
            LogLevel::Error => "error",
        }
    }
}

/// Progress sink used by all service jobs.
///
/// CLI passes [`StdoutReporter`]; the WebUI passes [`CaptureReporter`],
/// which records events for SSE streaming and later inspection.
pub trait Reporter: Send + Sync {
    /// A regular log line (implicitly [`LogLevel::Info`]).
    fn log(&self, msg: String);

    /// Log line with an explicit level. Default keeps custom reporters
    /// working by prefixing the message; the built-in reporters override it
    /// to emit structured events.
    fn log_at(&self, level: LogLevel, msg: String) {
        match level {
            LogLevel::Warn => self.log(format!("警告：{msg}")),
            LogLevel::Error => self.log(format!("错误：{msg}")),
            LogLevel::Debug | LogLevel::Info => self.log(msg),
        }
    }

    fn warn(&self, msg: String) {
        self.log_at(LogLevel::Warn, msg);
    }

    fn error(&self, msg: String) {
        self.log_at(LogLevel::Error, msg);
    }

    /// A major phase transition (used as progress milestones).
    fn stage(&self, name: &str);

    /// A numbered orchestration step (`index` 1-based). Composite jobs use
    /// this so the WebUI can render "步骤 2/4" and nest the sub-stages.
    fn step(&self, _index: usize, _total: usize, name: &str) {
        self.stage(name);
    }

    /// Cheap machine-readable progress signal for batch loops (no log line).
    /// Call sites should emit at a bounded rate (e.g. every 10 items).
    fn progress(&self, _label: &str, _done: usize, _total: usize) {}

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

    fn step(&self, index: usize, total: usize, name: &str) {
        println!("\n========== [{index}/{total}] {name} ==========");
    }

    /// CLI 侧保留可读的进度行，但按 10% 节流（每 20 条打印一次）。
    fn progress(&self, label: &str, done: usize, total: usize) {
        if total == 0 {
            return;
        }
        if done == total || done.is_multiple_of(20) {
            println!("{} {}/{}", label, done, total);
        }
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
        level: LogLevel,
        text: String,
    },
    /// Phase transition. `index`/`total` are set by [`Reporter::step`]
    /// (composite jobs) and `None` for ordinary sub-stages.
    Stage {
        seq: u64,
        ts_ms: u64,
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        index: Option<usize>,
        #[serde(skip_serializing_if = "Option::is_none")]
        total: Option<usize>,
    },
    /// Batch progress (no log line): `done` of `total` items.
    Progress {
        seq: u64,
        ts_ms: u64,
        label: String,
        done: usize,
        #[serde(skip_serializing_if = "Option::is_none")]
        total: Option<usize>,
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
        self.log_at(LogLevel::Info, msg);
    }

    fn log_at(&self, level: LogLevel, msg: String) {
        self.push(JobEvent::Log {
            seq: self.next_seq(),
            ts_ms: now_ms(),
            level,
            text: msg,
        });
    }

    fn stage(&self, name: &str) {
        self.push(JobEvent::Stage {
            seq: self.next_seq(),
            ts_ms: now_ms(),
            name: name.to_string(),
            index: None,
            total: None,
        });
    }

    fn step(&self, index: usize, total: usize, name: &str) {
        self.push(JobEvent::Stage {
            seq: self.next_seq(),
            ts_ms: now_ms(),
            name: name.to_string(),
            index: Some(index),
            total: Some(total),
        });
    }

    fn progress(&self, label: &str, done: usize, total: usize) {
        self.push(JobEvent::Progress {
            seq: self.next_seq(),
            ts_ms: now_ms(),
            label: label.to_string(),
            done,
            total: Some(total),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_reporter_structures_levels_steps_and_progress() {
        let r = CaptureReporter::new(true);
        r.log("plain".into());
        r.warn("slow".into());
        r.step(2, 4, "images-sync");
        r.progress("bucket", 10, 100);

        let events = r.snapshot_logs();
        let json: Vec<serde_json::Value> = events
            .iter()
            .map(|e| serde_json::to_value(e).unwrap())
            .collect();

        assert_eq!(json[0]["type"], "log");
        assert_eq!(json[0]["level"], "info");
        assert_eq!(json[1]["level"], "warn");
        assert_eq!(json[2]["type"], "stage");
        assert_eq!(json[2]["index"], 2);
        assert_eq!(json[2]["total"], 4);
        assert_eq!(json[3]["type"], "progress");
        assert_eq!(json[3]["done"], 10);
        assert_eq!(json[3]["total"], 100);
    }

    /// 自定义 Reporter 不实现 log_at 时仍然拿到带前缀的文本（向后兼容）。
    #[test]
    fn default_log_at_prefixes_warn_for_custom_reporters() {
        struct R(std::sync::Mutex<Vec<String>>);
        impl Reporter for R {
            fn log(&self, msg: String) {
                self.0.lock().unwrap().push(msg);
            }
            fn stage(&self, _name: &str) {}
            fn diff(&self, _page: &str, _text: &str, _added: usize, _removed: usize) {}
            fn confirm(&self, _prompt: &str) -> bool {
                false
            }
        }

        let r = R(std::sync::Mutex::new(Vec::new()));
        r.warn("x".into());
        r.error("y".into());
        let logs = r.0.lock().unwrap();
        assert_eq!(logs[0], "警告：x");
        assert_eq!(logs[1], "错误：y");
    }
}
