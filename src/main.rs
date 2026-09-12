mod commands;
mod web;

use tracing::Instrument;

#[tokio::main]
async fn main() {
    // dotenv must run before tracing init so RUST_LOG from .env is honored.
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    let run_id = uuid::Uuid::new_v4();

    // clap 在参数错误/--help 时自行退出；构造错误（非 UTF-8 路径等）在此兜底。
    let top = match commands::parse() {
        Ok(top) => top,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(2);
        }
    };
    let command_name = match &top {
        commands::TopCommand::Serve { .. } => "serve",
        commands::TopCommand::Job(inv) => inv.kind.name(),
    };

    // Every log line inside carries the run id + command for correlation.
    async {
        if let Err(e) = commands::run(top).await {
            tracing::error!(error = %e, "command failed");
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
    .instrument(tracing::info_span!("cli_run", %run_id, command = command_name))
    .await;
}
