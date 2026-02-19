# `jl` - JSON Log Pretty-Printer CLI

## Overview

Build a Rust CLI tool that reads JSON log lines from stdin or files and renders them as human-readable, colorized terminal output. Supports multiple logging schemas (Logstash, Logrus, Bunyan, Generic) with auto-detection, configurable output templates, level filtering, timezone conversion, and follow mode.

## Context

- Files involved: All files are new. Starting from a bare `Cargo.toml` and hello-world `main.rs`.
- Related patterns: None (greenfield project)
- Dependencies: clap 4.5, serde/serde_json, owo-colors 4.2, is-terminal 0.4, chrono 0.4, chrono-tz 0.10
- Dev dependencies: assert_cmd 2.0, predicates 3.1, tempfile 3.15
- Note: Cargo.toml uses `edition = "2024"`

## Development Approach

- **Testing approach**: TDD where practical - write tests alongside each module
- Complete each task fully before moving to the next
- Each module gets unit tests in a `#[cfg(test)]` block
- **CRITICAL: every task MUST include new/updated tests**
- **CRITICAL: all tests must pass before starting next task**

## Implementation Steps

### Task 1: Project setup and foundational types

**Files:**
- Modify: `Cargo.toml`
- Create: `src/error.rs`
- Create: `src/level.rs`
- Modify: `src/main.rs`

- [x] Add all dependencies and dev-dependencies to `Cargo.toml`
- [x] Create `src/error.rs` with `JlError` enum: `Io(std::io::Error)`, `Json(serde_json::Error)`, `Parse(String)`, `Tz(String)`. Implement `Display`, `std::error::Error`, and `From` impls for `io::Error` and `serde_json::Error`
- [x] Create `src/level.rs` with `Level` enum: `Trace, Debug, Info, Warn, Error, Fatal`. Implement `Ord`, `Display`, `FromStr` (case-insensitive), and a `from_bunyan_int(i64) -> Option<Level>` for Bunyan numeric levels (10=Trace, 20=Debug, 30=Info, 40=Warn, 50=Error, 60=Fatal)
- [x] Update `src/main.rs` to declare modules (`mod error; mod level;`)
- [x] Write unit tests for `Level` parsing (string variants, case insensitivity, Bunyan ints, ordering)
- [x] Write unit tests for `JlError` Display and From conversions
- [x] `cargo test` - must pass

### Task 2: CLI argument parsing

**Files:**
- Create: `src/cli.rs`
- Modify: `src/main.rs`

- [x] Create `src/cli.rs` with clap derive `Args` struct containing all CLI options: `format` (String, default template), `add_fields` (Option<String>), `omit_fields` (Option<String>), `color` (ColorMode enum: Auto/Always/Never), `non_json` (NonJsonMode enum: PrintAsIs/Skip/Fail), `schema` (SchemaChoice enum: Auto/Logstash/Logrus/Bunyan/Generic), `min_level` (Option<Level>), `raw_json` (bool), `compact` (bool), `tz` (String, default "local"), `follow` (bool), `output` (Option<PathBuf>), `files` (Vec<PathBuf> positional)
- [x] Implement `ValueEnum` for `ColorMode`, `NonJsonMode`, `SchemaChoice`; implement `FromStr` for `Level` to work with clap
- [x] Update `src/main.rs` to add `mod cli;` and parse args with `Args::parse()`
- [x] Write tests verifying default values and parsing of various option combinations
- [x] `cargo test` - must pass

### Task 3: Input and output abstractions

**Files:**
- Create: `src/input.rs`
- Create: `src/output.rs`
- Modify: `src/main.rs`

- [x] Create `src/input.rs` with `LineSource` trait (`fn next_line(&mut self) -> Result<Option<String>, JlError>`), `StdinSource` (wraps `BufReader<Stdin>`), `FileSource` (wraps `BufReader<File>`)
- [x] Create `src/output.rs` with `OutputSink` trait (`fn write_line(&mut self, line: &str) -> Result<(), JlError>`), `StdoutSink` (wraps `BufWriter<Stdout>`), `FileSink` (wraps `BufWriter<File>`)
- [x] Update `src/main.rs` to declare both modules
- [x] Write tests for `FileSource` reading lines from a temp file (using `tempfile`)
- [x] Write tests for `FileSink` writing lines to a temp file
- [x] `cargo test` - must pass

### Task 4: JSON parsing and non-JSON handling

**Files:**
- Create: `src/parse.rs`
- Modify: `src/main.rs`

- [x] Create `src/parse.rs` with `parse_line(line: &str, mode: NonJsonMode) -> Result<ParseResult, JlError>` where `ParseResult` is `Json(serde_json::Value)` | `NonJson(String)` | `Skip`
- [x] For `NonJsonMode::PrintAsIs`, non-JSON lines return `NonJson(line)`; for `Skip`, return `Skip`; for `Fail`, return `Err`
- [x] Write tests: valid JSON parsing, non-JSON with each mode, empty lines, partial JSON
- [x] `cargo test` - must pass

### Task 5: Schema detection and field mapping

**Files:**
- Create: `src/schema.rs`
- Modify: `src/main.rs`

- [x] Create `src/schema.rs` with `Schema` enum (Logstash, Logrus, Bunyan, Generic) and `FieldMapping` struct mapping canonical roles (level, timestamp, logger, message, stack_trace) to actual JSON key name(s) for each schema
- [x] Implement `detect_schema(value: &serde_json::Value) -> Schema`: score each schema by counting matching field names in the JSON object. Bunyan bonus if `level` is numeric and `v` exists. Logstash bonus if `@timestamp` exists. Highest score wins; fallback to Generic
- [x] Implement `Schema::field_mapping(&self) -> FieldMapping` returning the appropriate key mappings
- [x] For Generic schema, implement fallback logic trying multiple common field name variants for each role (e.g., "message", "msg", "text" for message; "level", "severity", "loglevel" for level; etc.)
- [x] Write tests: detection with Logstash fields, Logrus fields, Bunyan fields (numeric level + v), ambiguous objects falling to Generic, forced schema selection
- [x] `cargo test` - must pass

### Task 6: Log record extraction and timestamp handling

**Files:**
- Create: `src/record.rs`
- Create: `src/timestamp.rs`
- Modify: `src/main.rs`

- [ ] Create `src/timestamp.rs` with `parse_timestamp(value: &serde_json::Value) -> Option<chrono::DateTime<chrono::FixedOffset>>` supporting ISO 8601 strings, epoch seconds (f64/i64), and epoch millis. Add `format_timestamp(ts: &DateTime<FixedOffset>, tz: &str) -> Result<String, JlError>` converting to local/utc/IANA timezone
- [ ] Create `src/record.rs` with `LogRecord` struct: `level: Option<Level>`, `timestamp: Option<String>` (formatted), `logger: Option<String>`, `message: Option<String>`, `stack_trace: Option<String>`, `extras: BTreeMap<String, serde_json::Value>`, `raw: serde_json::Value`
- [ ] Implement `LogRecord::extract(value: serde_json::Value, mapping: &FieldMapping, tz: &str) -> Result<LogRecord, JlError>`: pull canonical fields using mapping, parse level (string or Bunyan int), format timestamp, collect remaining fields as extras
- [ ] Write tests for timestamp parsing (ISO 8601, epoch seconds, epoch millis, invalid)
- [ ] Write tests for timezone conversion (UTC, local, named timezone)
- [ ] Write tests for LogRecord extraction with each schema's field mapping
- [ ] `cargo test` - must pass

### Task 7: Color support and output formatting

**Files:**
- Create: `src/color.rs`
- Create: `src/format.rs`
- Modify: `src/main.rs`

- [ ] Create `src/color.rs` with `ColorConfig` struct: `enabled: bool` + `level_color(level: &Level) -> Style` mapping (Trace=dim, Debug=blue, Info=green, Warn=yellow, Error=red, Fatal=red+bold). Use `owo-colors` for styling. Determine `enabled` from `ColorMode` + `is-terminal`
- [ ] Create `src/format.rs` with `FormatToken` enum: `Literal(String)`, `Field(CanonicalField)`, `CustomField(String)`. Implement `parse_template(template: &str) -> Vec<FormatToken>` parsing `{field_name}` placeholders
- [ ] Implement `render(record: &LogRecord, tokens: &[FormatToken], color: &ColorConfig, args: &Args) -> String`: substitute fields, apply colors to level, handle `--add-fields`/`--omit-fields`, `--compact` (extras on same line), `--raw-json` (output as JSON), and stack trace appending
- [ ] Write tests for template parsing (default template, custom fields, literals only, adjacent fields)
- [ ] Write tests for rendering with and without color, with extras, with omit/add fields, compact mode
- [ ] `cargo test` - must pass

### Task 8: Pipeline and main entry point

**Files:**
- Create: `src/pipeline.rs`
- Create: `src/lib.rs`
- Modify: `src/main.rs`

- [ ] Create `src/pipeline.rs` with `run(args: Args) -> Result<(), JlError>`: construct input source(s) and output sink from args, parse template once, detect schema on first JSON line and cache, loop reading lines through parse -> extract -> level filter -> render -> write
- [ ] Handle multiple input files by processing them sequentially
- [ ] Handle `--min-level` filtering: skip records where `record.level < min_level`
- [ ] Create `src/lib.rs` re-exporting all modules for integration test access
- [ ] Update `src/main.rs` to call `pipeline::run()` with parsed args, print errors to stderr, exit with code 1 on error
- [ ] Write unit tests for pipeline logic: level filtering, schema caching, non-JSON pass-through
- [ ] `cargo test` - must pass

### Task 9: Follow mode and stack trace formatting

**Files:**
- Modify: `src/input.rs`
- Modify: `src/format.rs`

- [ ] Add `FollowSource` to `src/input.rs`: tail a file, sleeping briefly and retrying when EOF is reached (loop with `thread::sleep` + re-read), return lines as they appear
- [ ] Add stack trace pretty-printing in `src/format.rs`: when `stack_trace` is present, append it on new lines after the main log line, indented, optionally dimmed
- [ ] Write tests for stack trace formatting (multiline, indentation)
- [ ] `cargo test` - must pass

### Task 10: Integration tests

**Files:**
- Create: `tests/cli_tests.rs`

- [ ] Write integration tests using `assert_cmd` and `predicates`:
  - Pipe Logstash JSON to stdin, verify colorized output contains expected fields
  - Pipe Bunyan JSON (numeric level), verify correct level name in output
  - Pipe non-JSON line with `--non-json skip`, verify it is omitted
  - Pipe non-JSON line with `--non-json print-as-is`, verify it passes through
  - Use `--min-level WARN` and verify INFO lines are filtered out
  - Use `--schema logrus` to force schema, verify correct field extraction
  - Test `--color never` produces no ANSI codes
  - Test `--format` with custom template
  - Test file input (write temp file, pass as positional arg)
  - Test `-o` output file option
- [ ] `cargo test` - must pass

### Task 11: Verify acceptance criteria

- [ ] Manual test: `echo '{"@timestamp":"2024-01-15T10:30:00Z","level":"INFO","logger_name":"com.example","message":"hello"}' | cargo run`
- [ ] Manual test: `echo '{"level":30,"time":"2024-01-15T10:30:00Z","name":"myapp","msg":"started","v":0}' | cargo run` (Bunyan)
- [ ] Run full test suite: `cargo test`
- [ ] Run linter: `cargo clippy -- -D warnings`
- [ ] Run formatter check: `cargo fmt -- --check`

### Task 12: Update documentation

- [ ] Update README.md with usage, examples, and supported schemas
- [ ] Move this plan to `docs/plans/completed/`
