mod cli;
mod color;
mod error;
mod format;
mod input;
mod level;
mod output;
mod parse;
mod pipeline;
mod record;
mod schema;
mod timestamp;

use clap::Parser;

use cli::Args;

fn main() {
    let args = Args::parse();
    if let Err(e) = pipeline::run(args) {
        eprintln!("jl: {e}");
        std::process::exit(1);
    }
}
