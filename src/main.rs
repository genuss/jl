mod cli;
#[allow(dead_code)]
mod error;
#[allow(dead_code)]
mod level;

use clap::Parser;

use cli::Args;

fn main() {
    let _args = Args::parse();
}
