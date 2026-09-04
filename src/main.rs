#[cfg(feature = "cli")]
mod cli;
#[cfg(feature = "gui")]
mod ui;

fn main() {
    #[cfg(feature = "cli")]
    {
        use clap::Parser;
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
