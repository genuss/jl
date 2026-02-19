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
        if let error::JlError::Io(ref io_err) = e
            && io_err.kind() == std::io::ErrorKind::BrokenPipe
        {
            std::process::exit(0);
        }
        eprintln!("jl: {e}");
        std::process::exit(1);
    }
}
