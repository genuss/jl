# Plan: `jl` — JSON Log Pretty-Printer CLI in Rust

## Context

Build a CLI tool that reads JSON log lines (from stdin or files) and renders them as human-readable, colorized output. Inspired by [mightyguava/jl](https://github.com/mightyguava/jl) (Go), but written in Rust with richer schema detection and configurability.

---

## Module Structure

```
src/
  main.rs         — Entry point: parse args, call pipeline::run()
  lib.rs          — Re-exports for integration tests
  cli.rs          — clap derive Args struct + enums (ColorMode, NonJsonMode, SchemaChoice)
  error.rs        — JlError enum with From impls
  level.rs        — Level enum (Trace..Fatal), parsing from string/Bunyan int, Ord
  input.rs        — LineSource trait + StdinSource, FileSource, FollowSource
  output.rs       — OutputSink trait + StdoutSink, FileSink
  parse.rs        — JSON parsing + non-JSON policy (print-as-is / skip / fail)
  schema.rs       — Schema enum + FieldMapping + auto-detection scoring
  record.rs       — LogRecord struct + extract() from Value + FieldMapping
  timestamp.rs    — Timestamp parsing (ISO 8601, epoch), timezone conversion
  color.rs        — ColorConfig + level-to-ANSI mapping
  format.rs       — Template parsing ({timestamp} [{level}]...) + render()
  pipeline.rs     — Main loop: read → parse → detect → extract → filter → render → write
```

---

## CLI Interface

```
jl [OPTIONS] [FILES...]

Options:
  -F, --format <FMT>          Output template (default: "{timestamp} [{level}] {logger} - {message}")
      --add-fields <f1,f2>    Extra fields to append to output
      --omit-fields <f1,f2>   Fields to suppress from output
      --color <MODE>           auto | always | never (default: auto)
      --non-json <MODE>        print-as-is | skip | fail (default: print-as-is)
      --schema <SCHEMA>        auto | logstash | logrus | bunyan | generic (default: auto)
      --min-level <LEVEL>      Minimum log level to display (e.g. WARN)
      --raw-json               Output selected fields as JSON
      --compact                Extra fields on same line
      --tz <TZ>                local | utc | <IANA name> (default: local)
  -f, --follow                 Tail mode for file input
  -o, --output <FILE>          Write to file instead of stdout
```

---

## Supported Schemas

| Schema | Level | Timestamp | Logger | Message | Stack Trace |
|--------|-------|-----------|--------|---------|-------------|
| Logstash | `level` | `@timestamp` | `logger_name` | `message` | `stack_trace`, `exception` |
| Logrus | `level` | `time` | `component`/`source` | `msg` | `stack`, `error` |
| Bunyan | `level` (int) | `time` | `name` | `msg` | `err.stack` |
| Generic | many variants tried | many variants tried | many variants tried | many variants tried | many variants tried |

**Detection**: Score each schema by counting how many of its expected field names appear in the first JSON object. Bunyan gets a bonus if `level` is numeric and `v` exists. Logstash gets a bonus if `@timestamp` exists. Detected once, cached for all subsequent lines.

---

## Key Data Structures

- **`Args`** — clap derive struct with all CLI options
- **`Level`** — `Trace < Debug < Info < Warn < Error < Fatal`, parsed case-insensitively, supports Bunyan ints (10/20/30/40/50/60)
- **`Schema` + `FieldMapping`** — maps canonical roles (level, timestamp, logger, message, stack_trace) to actual JSON key names
- **`LogRecord`** — normalized struct with canonical fields + `BTreeMap<String, Value>` for extras + raw `Value`
- **`FormatToken`** — parsed template: `Literal(String)` | `Field(CanonicalField)` | `CustomField(String)`
- **`ColorConfig`** — color enabled flag + level→color map

---

## Processing Pipeline

```
stdin/file → read line → parse JSON → detect schema (1st line, then cached)
  → extract LogRecord → level filter → render via template + color → write to stdout/file
```

Non-JSON lines: handled per `--non-json` mode (pass-through / skip / error).

---

## Dependencies

```toml
[dependencies]
clap = { version = "4.5", features = ["derive"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
owo-colors = "4.2"
is-terminal = "0.4"
chrono = { version = "0.4", features = ["serde"] }
chrono-tz = "0.10"

[dev-dependencies]
assert_cmd = "2.0"
predicates = "3.1"
tempfile = "3.15"
```

---

## Implementation Order

### Phase 1: Foundation
1. `Cargo.toml` — add all dependencies
2. `error.rs` — `JlError` enum
3. `level.rs` — Level enum with parsing + ordering
4. `cli.rs` — Args struct with clap derive

### Phase 2: I/O
5. `input.rs` — LineSource trait + stdin/file sources
6. `output.rs` — OutputSink trait + stdout/file sinks

### Phase 3: Core Processing
7. `parse.rs` — JSON parsing + non-JSON policy
8. `schema.rs` — Schema detection + field mapping
9. `record.rs` — LogRecord extraction
10. `timestamp.rs` — Timestamp parsing + timezone conversion

### Phase 4: Rendering
11. `color.rs` — ANSI color support
12. `format.rs` — Template parsing + rendering

### Phase 5: Integration
13. `pipeline.rs` — Main processing loop
14. `main.rs` + `lib.rs` — Entry point and re-exports

### Phase 6: Polish
15. Follow mode in `input.rs`
16. Stack trace pretty-printing
17. Integration tests

---

## Verification

1. **Unit tests** in each module (`#[cfg(test)]`)
2. **Integration tests** using `assert_cmd`: pipe JSON stdin, verify stdout output
3. **Manual test**: `echo '{"@timestamp":"2024-01-15T10:30:00Z","level":"INFO","logger_name":"com.example","message":"hello"}' | cargo run`
4. **Build check**: `cargo build && cargo test && cargo clippy`
