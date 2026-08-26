//! WebUI server: axum router, static assets, and startup.

pub mod api_data;
pub mod api_jobs;
pub mod assets;
pub mod jobs;
pub mod state;

use axum::http::{header, StatusCode};
use axum::middleware::{self, Next};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use state::AppState;
use std::sync::Arc;

pub async fn serve(host: String, port: u16) -> dst_huiji_wiki::error::Result<()> {
    let app_state = Arc::new(AppState::new());

    // Warm up the dataset in the background so first page load is instant.
    {
        let state = Arc::clone(&app_state);
        tokio::spawn(async move {
            match state.datasets.get_or_load(None).await {
                Ok(ds) => tracing::info!(
                    "dataset loaded: {} recipes, {} po entries ({})",
                    ds.recipes.len(),
                    ds.po_total_entries,
                    ds.label
                ),
                Err(e) => tracing::warn!("dataset warm-up failed: {}", e),
            }
        });
    }

    let api = Router::new()
        .route("/jobs", post(api_jobs::submit).get(api_jobs::list))
        .route("/jobs/{id}", get(api_jobs::get))
        .route("/jobs/{id}/cancel", post(api_jobs::cancel))
        .route("/jobs/{id}/events", get(api_jobs::events))
        .route("/data/meta", get(api_data::meta))
        .route("/data/recipes", get(api_data::recipes))
        .route("/data/ingredients", get(api_data::ingredients))
        .route("/data/po/entries", get(api_data::po_entries))
        .route("/data/constants", get(api_data::constants))
        .route("/viz/skilltree", get(api_data::skilltree))
        .route("/snapshots", get(api_data::snapshots))
        .route("/diff/recipes", get(api_data::diff_recipes))
        .route("/diff/po", get(api_data::diff_po))
        .route("/config", get(config))
        .with_state(Arc::clone(&app_state));

    let router = Router::new()
        .route("/", get(index_handler))
        .route("/static/app.js", get(app_js_handler))
        .route("/static/style.css", get(style_css_handler))
        .nest("/api", api)
        .fallback(not_found)
        .layer(middleware::from_fn(log_requests))
        .with_state(app_state)
        .layer(tower_http::compression::CompressionLayer::new().gzip(true));

    let addr = format!("{}:{}", host, port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(dst_huiji_wiki::error::Error::Io)?;

    println!("WebUI 已启动: http://{}", addr);
    println!("按 Ctrl+C 停止服务");

    axum::serve(listener, router).await.map_err(|e| {
        dst_huiji_wiki::error::Error::Io(std::io::Error::other(format!("server: {}", e)))
    })?;

    Ok(())
}

async fn config() -> impl IntoResponse {
    let snapshots = dst_huiji_wiki::DstContext::list_snapshots();
    Json(serde_json::json!({
        "dst_root_set": std::env::var("DST__ROOT").is_ok(),
        "snapshots": snapshots,
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

async fn index_handler() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        assets::INDEX_HTML,
    )
}

async fn app_js_handler() -> impl IntoResponse {
    (
        [(
            header::CONTENT_TYPE,
            "application/javascript; charset=utf-8",
        )],
        assets::APP_JS,
    )
}

async fn style_css_handler() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        assets::STYLE_CSS,
    )
}

async fn not_found() -> (StatusCode, &'static str) {
    (StatusCode::NOT_FOUND, "404 Not Found")
}

async fn log_requests(
    req: axum::extract::Request,
    next: Next,
) -> Result<axum::response::Response, StatusCode> {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let start = std::time::Instant::now();
    let res = next.run(req).await;
    tracing::debug!(
        "{} {} -> {} ({:?})",
        method,
        path,
        res.status(),
        start.elapsed()
    );
    Ok(res)
}
