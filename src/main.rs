mod commands;
mod web;

use clap::Parser;
use tracing::Instrument;

#[tokio::main]
async fn main() {
    // dotenv must run before tracing init so RUST_LOG from .env is honored.
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    let args = commands::Args::parse();
    let run_id = uuid::Uuid::new_v4();
    let command_name = args.command.name();

    // Every log line inside carries the run id + command for correlation.
    async {
        if let Err(e) = commands::run(args.command).await {
            tracing::error!(error = %e, "command failed");
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
    .instrument(tracing::info_span!("cli_run", %run_id, command = command_name))
    .await;
}
