mod commands;

use clap::Parser;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    dotenvy::dotenv().ok();

    let args = commands::Args::parse();

    if let Err(e) = commands::run(args.command).await {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
