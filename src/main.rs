mod cli;
#[allow(dead_code)]
mod error;
#[allow(dead_code)]
mod input;
#[allow(dead_code)]
mod level;
#[allow(dead_code)]
mod output;
#[allow(dead_code)]
mod parse;
#[allow(dead_code)]
mod schema;

use clap::Parser;

use cli::Args;

fn main() {
    let _args = Args::parse();
}
