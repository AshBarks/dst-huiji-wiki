#[cfg(feature = "cli")]
mod cli;
#[cfg(feature = "gui")]
mod ui;

fn main() {
    #[cfg(feature = "cli")]
    {
        use clap::Parser;
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::from_default_env()
                    .add_directive(tracing::Level::WARN.into()),
            )
            .with_target(false)
            .init();
        let cli = cli::Cli::parse();
        if let Err(e) = cli::run(cli) {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }

    #[cfg(not(feature = "cli"))]
    {
        eprintln!("dst-anim-tool binary requires the 'cli' feature (enable default features)");
        std::process::exit(1);
    }
}
