# Add Shell Completions for bash, zsh, fish

## Overview

Add a `--completions <SHELL>` option to `jl` that generates shell completion scripts for bash, zsh, and fish. When invoked, the command prints the completion script to stdout and exits, allowing users to source or install the output.

## Context

- Files involved: `Cargo.toml`, `src/cli.rs`, `src/main.rs`, `tests/cli_tests.rs`
- Related patterns: Uses clap derive macros with `ValueEnum` enums for CLI options
- Dependencies: `clap_complete` crate (companion to the existing `clap` dependency)

## Development Approach

- **Testing approach**: Regular (code first, then tests)
- Complete each task fully before moving to the next
- **CRITICAL: every task MUST include new/updated tests**
- **CRITICAL: all tests must pass before starting next task**

## Implementation Steps

### Task 1: Add clap_complete dependency and completions option

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/cli.rs`

- [x] Add `clap_complete = "4.5"` to `[dependencies]` in `Cargo.toml`
- [x] Add a `Shell` enum (bash, zsh, fish) deriving `ValueEnum` to `src/cli.rs`
- [x] Add `--completions <SHELL>` optional argument to `Args` struct in `src/cli.rs`
- [x] Write unit tests: parse `--completions bash`, `--completions zsh`, `--completions fish`, and verify default (None)
- [x] Run project test suite - must pass before task 2

### Task 2: Generate and print completions in main

**Files:**
- Modify: `src/main.rs`
- Modify: `tests/cli_tests.rs`

- [x] In `main()`, after parsing args, check if `args.completions` is `Some(shell)`
- [x] If set, use `clap_complete::generate()` to write the completion script for the requested shell to stdout, then exit 0
- [x] Write an integration test in `tests/cli_tests.rs` that runs `jl --completions bash` and verifies stdout contains expected completion content (e.g., contains the string "jl")
- [x] Run project test suite - must pass before task 3

### Task 3: Verify acceptance criteria

- [ ] Manual test: run `jl --completions bash`, `jl --completions zsh`, `jl --completions fish` and verify each produces valid shell-specific output
- [ ] Run full test suite: `cargo test`
- [ ] Run linter: `cargo clippy`

### Task 4: Update documentation

- [ ] Update README.md if user-facing changes
- [ ] Update CLAUDE.md if internal patterns changed
- [ ] Move this plan to `docs/plans/completed/`
