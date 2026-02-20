# CLAUDE.md - AI Knowledge Base for jl

## Project Overview

Rust CLI tool for pretty-printing JSON log lines. Binary name: `jl`.

## Build & Test Commands

- Build: `cargo build`
- Test: `cargo test`
- Lint: `cargo clippy`
- Run: `cargo run -- [OPTIONS] [FILES]`

## Project Structure

- `src/main.rs` - Entry point, handles `--completions` (shell completion generation via `clap_complete`) then delegates to `pipeline::run()`
- `src/cli.rs` - CLI argument definitions using clap derive macros with `ValueEnum` enums
- `src/pipeline.rs` - Main processing pipeline: read -> parse -> extract -> filter -> render -> write
- `src/parse.rs` - JSON line parsing, non-JSON handling
- `src/schema.rs` - Log schema detection (Logstash, Logrus, Bunyan, Generic) and field mapping
- `src/record.rs` - LogRecord extraction from parsed JSON using field mappings
- `src/format.rs` - Format template parsing and rendering, logger name transformations, control char sanitization
- `src/timestamp.rs` - Timestamp parsing (ISO 8601, epoch) and formatting with timezone conversion
- `src/color.rs` - ColorConfig for ANSI styling (level colors, separator dimming)
- `src/output.rs` - OutputSink trait with StdoutSink (line-buffered with per-line flush) and FileSink
- `src/input.rs` - LineSource trait with StdinSource, FileSource, FollowSource
- `src/level.rs` - Log level enum with ordering
- `src/error.rs` - Error types
- `src/lib.rs` - Public library re-exports (used by integration tests)
- `tests/cli_tests.rs` - Integration tests using `assert_cmd`

## Key Patterns & Conventions

- CLI enums use clap `ValueEnum` derive with `#[arg(long, value_enum, default_value_t = ...)]`
- Test helpers use `default_args()` functions that construct `Args` manually (not via clap parsing). Note: test defaults may differ from CLI defaults (e.g., `LoggerFormat::AsIs` and `logger_length: 0` in tests vs `ShortDots` and `30` in CLI) to keep existing tests stable.
- `RenderContext` is pre-computed once before the processing loop and reused across all records
- StdoutSink flushes after every line (no BufWriter) for low-latency piped output
- `ColorConfig::with_enabled(bool)` is a test-only constructor (`#[cfg(test)]`)
- Control character sanitization (`sanitize_control_chars`) is applied to all user-supplied data before rendering to prevent terminal escape injection
- BrokenPipe errors in `main.rs` exit with code 0 for correct UNIX pipe behavior
- Schema detection uses field-name scoring with bonuses for distinctive fields (`@timestamp` for Logstash, numeric `level` + `v` for Bunyan)
