mod cli;
#[allow(dead_code)]
mod error;
#[allow(dead_code)]
mod input;
#[allow(dead_code)]
mod level;
#[allow(dead_code)]
mod output;

use clap::Parser;

use cli::Args;

fn main() {
    let _args = Args::parse();
}
