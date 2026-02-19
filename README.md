# jl - JSON Log Pretty-Printer

A Rust CLI tool that reads JSON log lines from stdin or files and renders them as human-readable, colorized terminal output.

## Features

- Auto-detects log schema (Logstash, Logrus, Bunyan, Generic)
- Colorized output with per-level styling
- Configurable output format templates
- Level filtering with `--min-level`
- Timezone conversion (local, UTC, or any IANA timezone)
- Follow mode (`--follow`) for tailing files
- Non-JSON line handling (print as-is, skip, or fail)
- Compact extra fields display by default (opt into expanded multi-line with `--expanded`)
- Logger name abbreviation (`--logger-format short-dots`) and length limiting (`--logger-length`)
- Timestamp format options (`--ts-format time` for time-only, `full` for datetime)
- Colored separators in extra fields output
- Line-buffered output for immediate display when piped
- Raw JSON output mode
- File input or stdin piping

## Installation

```sh
cargo install --path .
```

## Usage

```
jl [OPTIONS] [FILES]...
```

Pipe JSON logs through stdin:

```sh
cat app.log | jl
```

Read from files:

```sh
jl app.log error.log
```

### Options

| Option | Description | Default |
|---|---|---|
| `-f, --format <TEMPLATE>` | Output format template with `{field}` placeholders | `{timestamp} {level} [{logger}] {message}` |
| `--color <MODE>` | Color mode: `auto`, `always`, `never` | `auto` |
| `--non-json <MODE>` | Non-JSON handling: `print-as-is`, `skip`, `fail` | `print-as-is` |
| `--schema <SCHEMA>` | Force schema: `auto`, `logstash`, `logrus`, `bunyan`, `generic` | `auto` |
| `--logger-format <FORMAT>` | Logger name format: `short-dots` (abbreviate segments), `as-is` | `short-dots` |
| `--logger-length <N>` | Maximum display length for logger names (crops from left) | `30` |
| `--ts-format <FORMAT>` | Timestamp format: `time` (HH:MM:SS.mmm), `full` (datetime) | `time` |
| `--min-level <LEVEL>` | Minimum log level to display | (none) |
| `--tz <TIMEZONE>` | Timezone: `local`, `utc`, or IANA name | `local` |
| `--add-fields <FIELDS>` | Comma-separated extra fields to include | (none) |
| `--omit-fields <FIELDS>` | Comma-separated fields to omit | (none) |
| `--expanded` | Show extra fields on separate lines (default is compact/same-line) | off |
| `--raw-json` | Output records as raw JSON | off |
| `--follow` | Follow input file, waiting for new data | off |
| `-o, --output <FILE>` | Write output to a file instead of stdout | (stdout) |

### Log Levels

Levels from lowest to highest: `trace`, `debug`, `info`, `warn`, `error`, `fatal`

Use `--min-level` to filter out lower levels:

```sh
jl --min-level warn app.log
```

## Supported Schemas

`jl` auto-detects the log format from the first JSON line. You can also force a schema with `--schema`.

### Logstash

Fields: `@timestamp`, `level`, `logger_name`, `message`, `stack_trace`

```json
{"@timestamp":"2024-01-15T10:30:00Z","level":"INFO","logger_name":"com.example.App","message":"Server started"}
```

### Logrus

Fields: `time`, `level`, `component`, `msg`

```json
{"time":"2024-01-15T10:30:00Z","level":"info","component":"web","msg":"Request handled"}
```

### Bunyan

Fields: `time`, `level` (numeric), `name`, `msg`, `v`

Bunyan uses numeric levels: 10=trace, 20=debug, 30=info, 40=warn, 50=error, 60=fatal.

```json
{"time":"2024-01-15T10:30:00Z","level":30,"name":"myapp","msg":"Connection established","v":0}
```

### Generic

Falls back to trying common field name variants for each role:

- Message: `message`, `msg`, `text`, `body`, `log`
- Level: `level`, `severity`, `loglevel`, `log_level`, `lvl`
- Timestamp: `timestamp`, `@timestamp`, `time`, `ts`, `datetime`, `date`
- Logger: `logger`, `logger_name`, `name`, `component`, `source`, `caller`

## Examples

Basic usage with Logstash format:

```sh
echo '{"@timestamp":"2024-01-15T10:30:00Z","level":"INFO","logger_name":"com.example","message":"hello"}' | jl
```

Bunyan format with UTC timestamps:

```sh
echo '{"level":30,"time":"2024-01-15T10:30:00Z","name":"myapp","msg":"started","v":0}' | jl --tz utc
```

Custom format template:

```sh
jl --format "{level}: {message}" app.log
```

Filter warnings and above, no color:

```sh
jl --min-level warn --color never app.log
```

Follow a log file:

```sh
jl --follow /var/log/app.log
```

Extra fields (compact by default):

```sh
cat app.log | jl --add-fields host,pid
```

Expanded mode with specific fields:

```sh
cat app.log | jl --expanded --add-fields host,pid
```

Full logger names and datetime timestamps:

```sh
cat app.log | jl --logger-format as-is --ts-format full
```

## License

See [LICENSE](LICENSE) for details.
