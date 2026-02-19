mod cli;
#[allow(dead_code)]
mod color;
#[allow(dead_code)]
mod error;
#[allow(dead_code)]
mod format;
#[allow(dead_code)]
mod input;
#[allow(dead_code)]
mod level;
#[allow(dead_code)]
mod output;
#[allow(dead_code)]
mod parse;
#[allow(dead_code)]
mod record;
#[allow(dead_code)]
mod schema;
#[allow(dead_code)]
mod timestamp;

use clap::Parser;

use cli::Args;

fn main() {
    let _args = Args::parse();
}
