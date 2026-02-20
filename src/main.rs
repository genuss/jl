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

use clap::{CommandFactory, Parser};

use cli::Args;

fn main() {
    let args = Args::parse();

    if let Some(shell) = args.completions {
        let mut cmd = Args::command();
        let mut buf = Vec::new();
        match shell {
            cli::Shell::Bash => {
                clap_complete::aot::generate(clap_complete::aot::Bash, &mut cmd, "jl", &mut buf);
            }
            cli::Shell::Zsh => {
                clap_complete::aot::generate(clap_complete::aot::Zsh, &mut cmd, "jl", &mut buf);
            }
            cli::Shell::Fish => {
                clap_complete::aot::generate(clap_complete::aot::Fish, &mut cmd, "jl", &mut buf);
            }
        }
        let mut stdout = std::io::stdout();
        if let Err(e) = std::io::Write::write_all(&mut stdout, &buf)
            .and_then(|_| std::io::Write::flush(&mut stdout))
        {
            if e.kind() == std::io::ErrorKind::BrokenPipe {
                std::process::exit(0);
            }
            eprintln!("jl: {e}");
            std::process::exit(1);
        }
        return;
    }

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
