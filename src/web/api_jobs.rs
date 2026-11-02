//! Job submission / tracking endpoints (including the SSE event stream).

use super::jobs::JobStatus;
use super::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::Json;
use dst_huiji_wiki::platform::progress::JobEvent;
use dst_huiji_wiki::service::job_spec::{ParamDefault, ParamKind};
use dst_huiji_wiki::service::{JobKind, JOB_SPECS};
use futures_util::stream::{self, Stream};
use futures_util::StreamExt;
use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;
use tokio_stream::wrappers::BroadcastStream;
use uuid::Uuid;

#[derive(Debug, serde::Deserialize)]
pub struct SubmitJobRequest {
    #[serde(flatten)]
    pub kind: JobKind,
    /// When true (default), wiki-touching jobs run in `WriteMode::DryRun`
    /// (diffs only, no writes); false runs them in `AutoConfirm`. Only
    /// wiki-touching kinds are affected: local-only kinds (scripts_sync,
    /// images_sync, …) gate their own disk writes through `JobKind` fields of
    /// the same name — which stay reachable precisely because this field is
    /// *not* named `dry_run`.
    #[serde(default = "default_true")]
    pub wiki_dry_run: bool,
}

fn default_true() -> bool {
    true
}

fn parse_uuid(raw: &str) -> Result<Uuid, StatusCode> {
    Uuid::parse_str(raw).map_err(|_| StatusCode::NOT_FOUND)
}

/// POST /api/jobs
pub async fn submit(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SubmitJobRequest>,
) -> std::result::Result<Json<serde_json::Value>, StatusCode> {
    // Wiki-touching jobs default to wiki-dry-run; explicit confirmation flips it.
    let handle = state
        .jobs
        .submit(req.kind, req.wiki_dry_run, Some(Arc::clone(&state)))
        .await;
    Ok(Json(handle.summary().await))
}

/// WebUI 可提交作业的声明（`JobSpec` 单一真源），供前端生成下拉与通用表单。
///
/// 只有 `web_ui = true` 的作业会出现在这里；标签/是否写站/参数 schema
/// 均来自 `JOB_SPECS`，不再在前端重复维护。
pub(crate) fn specs_json() -> serde_json::Value {
    let jobs: Vec<serde_json::Value> = JOB_SPECS
        .iter()
        .filter(|s| s.web_ui)
        .map(|s| {
            let params: Vec<serde_json::Value> = s
                .params
                .iter()
                .map(|p| {
                    let kind = match p.kind {
                        ParamKind::Str => "str",
                        ParamKind::Path => "path",
                        ParamKind::OptStr => "opt_str",
                        ParamKind::OptPath => "opt_path",
                        ParamKind::Bool => "bool",
                        ParamKind::Usize => "usize",
                        ParamKind::StrList => "str_list",
                    };
                    serde_json::json!({
                        "key": p.key,
                        "kind": kind,
                        "default": default_json(p.default),
                        "required": p.required,
                        "help": p.help,
                    })
                })
                .collect();
            serde_json::json!({
                // `name` 是 CLI 的 kebab 名；`tag` 是 serde/Web JSON 的
                // snake 名，前端提交 `kind` 必须用后者。
                "name": s.name,
                "tag": s.name.replace('-', "_"),
                "label": s.label,
                "wiki": s.touches_wiki(),
                "params": params,
            })
        })
        .collect();
    serde_json::json!({ "jobs": jobs })
}

fn default_json(d: ParamDefault) -> serde_json::Value {
    match d {
        ParamDefault::None => serde_json::Value::Null,
        ParamDefault::Str(s) => serde_json::Value::from(s),
        ParamDefault::Int(n) => serde_json::Value::from(n),
        ParamDefault::Bool(b) => serde_json::Value::from(b),
    }
}

/// GET /api/jobs/specs
pub async fn specs() -> Json<serde_json::Value> {
    Json(specs_json())
}

/// GET /api/jobs
pub async fn list(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let jobs = state.jobs.list().await;
    let mut summaries = Vec::with_capacity(jobs.len());
    for j in &jobs {
        summaries.push(j.summary().await);
    }
    Json(serde_json::json!({ "jobs": summaries }))
}

/// GET /api/jobs/:id
pub async fn get(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> std::result::Result<Json<serde_json::Value>, StatusCode> {
    let id = parse_uuid(&id)?;
    match state.jobs.get(id).await {
        Some(job) => Ok(Json(job.detail().await)),
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// POST /api/jobs/:id/cancel
///
/// 取消仅对排队中（尚未开跑）的任务生效。运行中的任务不支持硬取消：
/// 丢弃 future 可能把批量上传/文件搬移留在中间态。
pub async fn cancel(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> std::result::Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let id = parse_uuid(&id)
        .map_err(|code| (code, Json(serde_json::json!({ "error": "invalid job id" }))))?;
    let job = state.jobs.get(id).await.ok_or(not_found("job not found"))?;
    if job.try_cancel_from_queue().await {
        return Ok(Json(serde_json::json!({ "cancelled": true })));
    }
    match job.status().await {
        JobStatus::Running => Err((
            StatusCode::CONFLICT,
            Json(serde_json::json!({
                "error": "任务已在运行中，不支持取消（避免中断批量写入的中间态），请等待其完成。"
            })),
        )),
        status => Ok(Json(serde_json::json!({
            "cancelled": false,
            "reason": "already_finished",
            "status": status,
        }))),
    }
}

fn not_found(msg: &str) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({ "error": msg })),
    )
}

fn event_to_sse(ev: JobEvent) -> Event {
    Event::default().data(serde_json::to_string(&ev).unwrap_or_default())
}

/// 事件的全局序号（CaptureReporter 内单调递增），作为 SSE 断线续传游标。
fn event_seq(ev: &JobEvent) -> u64 {
    match ev {
        JobEvent::Log { seq, .. }
        | JobEvent::Stage { seq, .. }
        | JobEvent::Progress { seq, .. }
        | JobEvent::Diff { seq, .. }
        | JobEvent::Done { seq, .. } => *seq,
    }
}

/// replay 侧过滤：只保留 `seq > since` 的事件，与 live 侧同一判据
/// （避免订阅-快照之间产生的事件在两侧重复）。
fn filter_events_since(events: impl IntoIterator<Item = JobEvent>, since: u64) -> Vec<JobEvent> {
    events
        .into_iter()
        .filter(|ev| event_seq(ev) > since)
        .collect()
}

enum LiveItem {
    Ev(JobEvent),
    /// 广播队列滞后、丢失了事件：通知前端 resync（重新拉取全量日志）。
    Resync,
}

/// GET /api/jobs/:id/events — live SSE stream of job events.
///
/// The stream replays buffered history first, then follows live events,
/// terminating right after the terminal `Done` marker.
///
/// `since` 是 **seq 游标**（事件单调序号），replay 与 live 两侧都按
/// `seq > since` 过滤——订阅与快照之间产生的事件不会再同时出现在两侧
/// （此前按条数跳过，前端去重不可靠）。广播队列滞后丢事件时发送
/// `{"type":"resync"}`，前端重新拉取全量日志，Done 丢失也能恢复。
pub async fn events(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(q): Query<HashMap<String, String>>,
) -> std::result::Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, StatusCode> {
    let id = parse_uuid(&id)?;
    let job = state.jobs.get(id).await.ok_or(StatusCode::NOT_FOUND)?;
    let since = q
        .get("since")
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);

    // Subscribe before reading status so no events are missed.
    let rx = job.reporter.subscribe();
    let already_terminal = job.status().await.is_terminal();

    let replay: Vec<JobEvent> = filter_events_since(job.reporter.snapshot_logs(), since);
    let replay_stream = stream::iter(replay.into_iter().map(|ev| Ok(event_to_sse(ev))));

    let live: std::pin::Pin<Box<dyn Stream<Item = Result<Event, Infallible>> + Send>> = {
        let finished = BroadcastStream::new(rx)
            .filter_map(move |res| async move {
                match res {
                    Ok(ev) if event_seq(&ev) > since => Some(LiveItem::Ev(ev)),
                    Ok(_) => None,
                    Err(tokio_stream::wrappers::errors::BroadcastStreamRecvError::Lagged(_)) => {
                        Some(LiveItem::Resync)
                    }
                }
            })
            .map(|item| match item {
                LiveItem::Ev(ev) => {
                    let done = matches!(ev, JobEvent::Done { .. });
                    (event_to_sse(ev), done)
                }
                LiveItem::Resync => (Event::default().data(r#"{"type":"resync"}"#), false),
            })
            // Emit everything, including the single Done marker, then stop.
            .scan(false, |done_seen, item| {
                let stop_after = item.1;
                let out = if *done_seen {
                    None
                } else {
                    Some(Ok::<_, Infallible>(item.0))
                };
                *done_seen = *done_seen || stop_after;
                std::future::ready(out)
            });
        Box::pin(finished)
    };

    let combined: std::pin::Pin<Box<dyn Stream<Item = Result<Event, Infallible>> + Send>> =
        if already_terminal {
            Box::pin(replay_stream)
        } else {
            Box::pin(replay_stream.chain(live))
        };

    let sse = Sse::new(combined).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    );
    Ok(sse)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn log(seq: u64) -> JobEvent {
        JobEvent::Log {
            seq,
            ts_ms: 0,
            level: dst_huiji_wiki::platform::progress::LogLevel::Info,
            text: format!("e{seq}"),
        }
    }

    fn done(seq: u64) -> JobEvent {
        JobEvent::Done {
            seq,
            ts_ms: 0,
            status: "success".into(),
        }
    }

    /// replay 游标是严格 `seq > since`：边界事件不重复，Done 不丢失。
    #[test]
    fn filter_since_uses_strict_seq_cursor() {
        let events = vec![log(0), log(1), log(2), done(3)];
        let seqs = |v: Vec<JobEvent>| v.iter().map(event_seq).collect::<Vec<_>>();
        assert_eq!(seqs(filter_events_since(events.clone(), 1)), vec![2, 3]);
        assert!(filter_events_since(events.clone(), 3).is_empty());
        assert_eq!(seqs(filter_events_since(events, 0)), vec![1, 2, 3]);
    }

    /// `/api/jobs/specs` 是 WebUI 表单的唯一真源：只暴露 web_ui 作业，
    /// 标签/wiki 标记/参数 schema 均来自 JOB_SPECS。
    #[test]
    fn specs_json_exposes_web_ui_jobs_and_param_schema() {
        let v = specs_json();
        let jobs = v["jobs"].as_array().expect("jobs array");
        let names: Vec<&str> = jobs.iter().filter_map(|j| j["name"].as_str()).collect();

        // 新暴露的通用表单作业与核心编排都在；LLM/调试类不暴露。
        for name in [
            "update-local",
            "update-data",
            "update-assets",
            "update-check",
            "update-scan",
        ] {
            assert!(names.contains(&name), "缺少 {name}");
        }
        // 前端提交的 kind 是 serde tag（snake），必须随 spec 一起下发。
        let tags: Vec<&str> = jobs.iter().filter_map(|j| j["tag"].as_str()).collect();
        assert!(tags.contains(&"update_scan"));
        assert!(tags.contains(&"maintain_item_table"));
        for name in [
            "parse-po",
            "corpus-fetch",
            "maintain-wikitext",
            "anim-index",
        ] {
            assert!(names.contains(&name), "缺少 {name}");
        }
        // LLM/知识线作业仍不在 WebUI 暴露（有成本、需单独确认）。
        for name in ["knowledge-sync", "knowledge-scan-symbols", "page-assist"] {
            assert!(!names.contains(&name), "不应暴露 {name}");
        }

        let local = jobs
            .iter()
            .find(|j| j["name"] == "update-local")
            .expect("update-local spec");
        assert_eq!(local["wiki"], false, "update-local 不写维基");
        assert!(!local["label"].as_str().unwrap().is_empty());
        let params = local["params"].as_array().expect("params");
        let from = params
            .iter()
            .find(|p| p["key"] == "from")
            .expect("from param");
        assert_eq!(from["kind"], "opt_str");
        assert_eq!(from["required"], false);
        let dry = params
            .iter()
            .find(|p| p["key"] == "dry_run")
            .expect("dry_run param");
        assert_eq!(dry["kind"], "bool");
        assert_eq!(dry["default"], false);

        let check = jobs
            .iter()
            .find(|j| j["name"] == "update-check")
            .expect("update-check spec");
        assert_eq!(check["wiki"], false);
        let skip = check["params"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["key"] == "skip_icon_status")
            .expect("skip_icon_status");
        assert_eq!(skip["kind"], "bool");
        assert_eq!(skip["default"], false);
    }
}
