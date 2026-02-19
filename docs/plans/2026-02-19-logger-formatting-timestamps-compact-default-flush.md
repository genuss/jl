# Logger formatting, timestamp options, compact default, colored k=v, and output flushing

## Overview

Seven enhancements to the `jl` JSON log pretty-printer:

  1. Logger name format argument (`--logger-format`: `short-dots` default, `as-is`)
  2. Logger name max length argument (`--logger-length`, default 30, crops from left)
  3. Timestamp format argument (`--ts-format`: `time` default, `full`)
  4. Remove timezone offset from formatted timestamps
  5. Make compact mode the default (flip `--compact` flag semantics)
  6. Colorize the `=` sign in k=v extra fields output
  7. Flush stdout after each line to reduce buffering latency

## Context

  - Files involved: `src/cli.rs`, `src/format.rs`, `src/timestamp.rs`, `src/pipeline.rs`, `src/output.rs`, `src/color.rs`, `src/main.rs`, `tests/cli_tests.rs`
  - Related patterns: clap derive macros with `ValueEnum` for enums, `#[arg(long, default_value_t)]` for defaults
  - Dependencies: no new external dependencies needed

## Development Approach

  - **Testing approach**: Regular (code first, then tests)
  - Complete each task fully before moving to the next
  - **CRITICAL: every task MUST include new/updated tests**
  - **CRITICAL: all tests must pass before starting next task**

## Implementation Steps

### Task 1: Add `--logger-format` argument and short-dots transformation

**Files:**

  - Modify: `src/cli.rs`
  - Modify: `src/format.rs`

  - [x] Add `LoggerFormat` enum to `cli.rs` with variants `ShortDots` and `AsIs`, deriving `ValueEnum`
  - [x] Add `logger_format` field to `Args` struct with `#[arg(long, value_enum, default_value_t = LoggerFormat::ShortDots)]`
  - [x] Add `shorten_logger_dots(name: &str) -> String` function in `format.rs` that transforms `com.example.logger` to `c.e.logger` (abbreviate all segments except the last to their first character)
  - [x] In `format.rs::render()`, when rendering `CanonicalField::Logger`, apply the logger format transformation based on `args.logger_format` before outputting
  - [x] Write unit tests for `shorten_logger_dots`: empty string, single segment, two segments, many segments, already short
  - [x] Write tests for render with `LoggerFormat::AsIs` and `LoggerFormat::ShortDots`
  - [x] Update `default_args()` helper in `format.rs` and `pipeline.rs` test modules to include the new field
  - [x] Run `cargo test` - must pass before task 2

### Task 2: Add `--logger-length` argument with left-crop

**Files:**

  - Modify: `src/cli.rs`
  - Modify: `src/format.rs`

  - [x] Add `logger_length` field to `Args` struct: `#[arg(long, default_value_t = 30)]` as `usize`
  - [x] Add `truncate_logger_left(name: &str, max_len: usize) -> String` function in `format.rs` that crops from the left side when the name exceeds max length (respecting dot boundaries when possible: strip leftmost `segment.` chunks until it fits, then hard-truncate if still too long)
  - [x] In `format.rs::render()`, apply logger length truncation after format transformation
  - [x] Write unit tests: name shorter than max (unchanged), name exactly at max, name longer than max with dot segments, name longer than max without dots
  - [x] Update `default_args()` helpers to include `logger_length`
  - [x] Run `cargo test` - must pass before task 3

### Task 3: Add `--ts-format` argument and time-only mode

**Files:**

  - Modify: `src/cli.rs`
  - Modify: `src/timestamp.rs`

  - [x] Add `TsFormat` enum to `cli.rs` with variants `Time` and `Full`, deriving `ValueEnum`
  - [x] Add `ts_format` field to `Args` struct with `#[arg(long, value_enum, default_value_t = TsFormat::Time)]`
  - [x] Modify `format_timestamp()` signature to accept `ts_format: TsFormat` parameter (or pass it through `Args`)
  - [x] When `TsFormat::Time`, format as `%H:%M:%S%.3f` (e.g., `10:30:00.123`); when `TsFormat::Full`, use current format but without timezone offset (see task 4)
  - [x] Update `record.rs` where `format_timestamp` is called to pass the ts_format arg
  - [x] Write tests for both time and full formats
  - [x] Update `default_args()` helpers to include `ts_format`
  - [x] Run `cargo test` - must pass before task 4

### Task 4: Remove timezone offset from timestamp output

**Files:**

  - Modify: `src/timestamp.rs`

  - [x] Change all format strings in `format_timestamp()` to omit the `%:z` suffix - use `%Y-%m-%dT%H:%M:%S%.3f` for `full` mode (instead of `%Y-%m-%dT%H:%M:%S%.3f%:z`), and remove the `Z` suffix for UTC mode too
  - [x] Update all existing timestamp tests to expect the new format without timezone offset
  - [x] Update integration tests in `tests/cli_tests.rs` that assert on timestamp format
  - [x] Run `cargo test` - must pass before task 5

### Task 5: Make compact mode the default

**Files:**

  - Modify: `src/cli.rs`
  - Modify: `src/format.rs`
  - Modify: `src/pipeline.rs`
  - Modify: `tests/cli_tests.rs`

  - [ ] Change the `compact` field in `Args` to default true: `#[arg(long, default_value_t = true)]`
  - [ ] Add `--no-compact` or rename to allow disabling: replace the boolean `compact` with a `CompactMode` enum or use `--expanded` flag for the non-compact mode. Simplest: rename to `--expanded` as the opt-in for multi-line extras, and make compact the implicit default
  - [ ] Update `format.rs::render()` to check the new flag accordingly (if using `--expanded`, check `args.expanded` instead of `!args.compact`)
  - [ ] Update all tests that set or check `compact` behavior
  - [ ] Run `cargo test` - must pass before task 6

### Task 6: Add color to `=` sign in k=v extra fields

**Files:**

  - Modify: `src/format.rs`
  - Modify: `src/color.rs`

  - [ ] Add a method to `ColorConfig` for styling the equals sign (e.g., `style_equals(&self) -> String` that returns a dimmed or cyan `=` when color is enabled, plain `=` otherwise)
  - [ ] In `format.rs::render()`, in the compact mode extras formatting, use the colored `=` instead of a plain string literal
  - [ ] Also apply to non-compact (expanded) mode's `key: value` - color the `:` separator with the same style for consistency
  - [ ] Write tests verifying ANSI codes appear in `=` when color is enabled and don't appear when disabled
  - [ ] Run `cargo test` - must pass before task 7

### Task 7: Flush stdout after every line for reduced buffering

**Files:**

  - Modify: `src/output.rs`
  - Modify: `src/pipeline.rs`

  - [ ] In `StdoutSink::write_line()`, call `self.writer.flush()` after each `writeln!()` call (line-buffered behavior). This ensures output appears immediately when piped from commands like `kubectl`
  - [ ] Remove the conditional `if args.follow { output.flush()? }` from `pipeline.rs::process_source()` since flushing now happens per line unconditionally in StdoutSink
  - [ ] Keep the explicit `output.flush()` at the end of `pipeline::run()` as a safety net
  - [ ] Verify existing tests still pass (flushing more often should not change correctness)
  - [ ] Run `cargo test` - must pass before task 8

### Task 8: Verify acceptance criteria

  - [ ] Manual test: `echo '{"@timestamp":"2024-01-15T10:30:00Z","level":"INFO","logger_name":"com.example.service.MyHandler","message":"request handled","host":"server1"}' | cargo run -- --add-fields host` and verify:
    - Logger shows `c.e.s.MyHandler` (short-dots, default)
    - Timestamp shows time only (default ts-format)
    - No timezone offset in timestamp
    - Compact mode is default (host=server1 on same line)
    - `=` sign is colored (when terminal)
  - [ ] Manual test: same with `--logger-format as-is --ts-format full --expanded` to verify override behavior
  - [ ] Manual test with `--logger-length 10` to verify left-cropping
  - [ ] Run full test suite: `cargo test`
  - [ ] Run clippy: `cargo clippy`

### Task 9: Update documentation

  - [ ] Update README.md with new CLI arguments and their defaults
  - [ ] Update CLAUDE.md if internal patterns changed
  - [ ] Move this plan to `docs/plans/completed/`
