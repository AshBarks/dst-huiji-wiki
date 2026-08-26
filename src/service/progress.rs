use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::broadcast;

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

    fn confirm(&self, _prompt: &str) -> bool {
        self.auto_confirm
    }
}
