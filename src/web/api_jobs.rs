//! Job submission / tracking endpoints (including the SSE event stream).

use super::state::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::Json;
use dst_huiji_wiki::service::{JobEvent, JobKind};
use futures_util::stream::{self, Stream};
use futures_util::StreamExt;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;
use tokio_stream::wrappers::BroadcastStream;
use uuid::Uuid;

#[derive(Debug, serde::Deserialize)]
pub struct SubmitJobRequest {
    #[serde(flatten)]
    pub kind: JobKind,
    /// When true (default), wiki writes are answered "no" (dry-run).
    #[serde(default = "default_true")]
    pub dry_run: bool,
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
    // Wiki-touching jobs default to dry-run; explicit confirmation flips it.
    let auto_confirm = !req.dry_run;
    let handle = state
        .jobs
        .submit(req.kind, auto_confirm, Some(Arc::clone(&state.datasets)))
        .await;
    Ok(Json(handle.summary().await))
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
pub async fn cancel(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> std::result::Result<Json<serde_json::Value>, StatusCode> {
    let id = parse_uuid(&id)?;
    match state.jobs.get(id).await {
        Some(job) => {
            job.cancel.cancel();
            Ok(Json(serde_json::json!({ "cancelling": true })))
        }
        None => Err(StatusCode::NOT_FOUND),
    }
}

fn event_to_sse(ev: JobEvent) -> Event {
    Event::default().data(serde_json::to_string(&ev).unwrap_or_default())
}

/// GET /api/jobs/:id/events — live SSE stream of job events.
///
/// The stream replays buffered history first, then follows live events,
/// terminating right after the terminal `Done` marker.
pub async fn events(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> std::result::Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, StatusCode> {
    let id = parse_uuid(&id)?;
    let job = state.jobs.get(id).await.ok_or(StatusCode::NOT_FOUND)?;

    // Subscribe before reading status so no events are missed.
    let rx = job.reporter.subscribe();
    let already_terminal = job.status().await.is_terminal();

    let replay: Vec<JobEvent> = job.reporter.snapshot_logs();
    let replay_stream = stream::iter(replay.into_iter().map(|ev| Ok(event_to_sse(ev))));

    let live: std::pin::Pin<Box<dyn Stream<Item = Result<Event, Infallible>> + Send>> = {
        let finished = BroadcastStream::new(rx)
            .filter_map(|res| async move { res.ok() })
            .map(|ev| {
                let payload = event_to_sse(ev.clone());
                (payload, matches!(ev, JobEvent::Done { .. }))
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
