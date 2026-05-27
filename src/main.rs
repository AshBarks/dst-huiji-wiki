mod anim;
mod archive;
mod atlas;
mod build_file;
mod cli;
mod error;
mod gif_export;
mod hash;
mod ktex;
mod reader;
mod render;
mod specs;
mod ui;
mod writer;
mod xor;

fn main() {
    use clap::Parser;
    let cli = cli::Cli::parse();
    if let Err(e) = cli::run(cli) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
