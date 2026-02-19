use crate::cli::Args;
use crate::color::ColorConfig;
use crate::error::JlError;
use crate::format;
use crate::input::{FileSource, FollowSource, LineSource, StdinSource};
use crate::output::{FileSink, OutputSink, StdoutSink};
use crate::parse::{self, ParseResult};
use crate::record::LogRecord;
use crate::schema::Schema;

/// Run the full pipeline: read lines, parse, extract, filter, render, write.
pub fn run(args: Args) -> Result<(), JlError> {
    let color = ColorConfig::new(args.color);
    let tokens = format::parse_template(&args.format);

    let mut output: Box<dyn OutputSink> = match &args.output {
        Some(path) => Box::new(FileSink::new(path)?),
        None => Box::new(StdoutSink::new()),
    };

    if args.files.is_empty() {
        let mut source = StdinSource::new();
        process_source(&mut source, &mut *output, &tokens, &color, &args)?;
    } else if args.follow {
        // Follow mode: tail the last file, reading existing content then waiting for new lines
        // Process any preceding files normally first
        for path in &args.files[..args.files.len().saturating_sub(1)] {
            let mut source = FileSource::new(path)?;
            process_source(&mut source, &mut *output, &tokens, &color, &args)?;
        }
        if let Some(path) = args.files.last() {
            let mut source = FollowSource::new(path)?;
            process_source(&mut source, &mut *output, &tokens, &color, &args)?;
        }
    } else {
        for path in &args.files {
            let mut source = FileSource::new(path)?;
            process_source(&mut source, &mut *output, &tokens, &color, &args)?;
        }
    }

    Ok(())
}

/// Process lines from a single source through the pipeline.
fn process_source(
    source: &mut dyn LineSource,
    output: &mut dyn OutputSink,
    tokens: &[format::FormatToken],
    color: &ColorConfig,
    args: &Args,
) -> Result<(), JlError> {
    let mut cached_schema: Option<Schema> = None;

    while let Some(line) = source.next_line()? {
        match parse::parse_line(&line, args.non_json)? {
            ParseResult::Json(value) => {
                // Detect or use cached schema
                let schema = match cached_schema {
                    Some(s) => s,
                    None => {
                        let s = Schema::from_choice(args.schema, &value);
                        cached_schema = Some(s);
                        s
                    }
                };

                let mapping = schema.field_mapping();
                let record = LogRecord::extract(value, &mapping, &args.tz)?;

                // Apply --min-level filter
                if let Some(ref min_level) = args.min_level {
                    match &record.level {
                        Some(level) if level < min_level => continue,
                        _ => {}
                    }
                }

                let rendered = format::render(&record, tokens, color, args);
                output.write_line(&rendered)?;
            }
            ParseResult::NonJson(text) => {
                output.write_line(&text)?;
            }
            ParseResult::Skip => {}
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{ColorMode, NonJsonMode, SchemaChoice};
    use crate::level::Level;
    use std::io::Write;
    use std::path::PathBuf;
    use tempfile::NamedTempFile;

    fn default_args() -> Args {
        Args {
            format: "{timestamp} {level} [{logger}] {message}".to_string(),
            add_fields: None,
            omit_fields: None,
            color: ColorMode::Never,
            non_json: NonJsonMode::PrintAsIs,
            schema: SchemaChoice::Auto,
            min_level: None,
            raw_json: false,
            compact: false,
            tz: "utc".to_string(),
            follow: false,
            output: None,
            files: vec![],
        }
    }

    fn write_input(lines: &[&str]) -> NamedTempFile {
        let mut tmp = NamedTempFile::new().unwrap();
        for line in lines {
            writeln!(tmp, "{line}").unwrap();
        }
        tmp.flush().unwrap();
        tmp
    }

    fn run_with_input(lines: &[&str], args_modifier: impl FnOnce(&mut Args)) -> String {
        let input = write_input(lines);
        let output = NamedTempFile::new().unwrap();
        let output_path = output.path().to_owned();

        let mut args = default_args();
        args.files = vec![input.path().to_path_buf()];
        args.output = Some(output_path.clone());
        args_modifier(&mut args);

        run(args).unwrap();
        std::fs::read_to_string(&output_path).unwrap()
    }

    // --- Level filtering tests ---

    #[test]
    fn min_level_filters_below() {
        let output = run_with_input(
            &[
                r#"{"@timestamp":"2024-01-15T10:30:00Z","level":"DEBUG","logger_name":"app","message":"debug msg"}"#,
                r#"{"@timestamp":"2024-01-15T10:30:00Z","level":"INFO","logger_name":"app","message":"info msg"}"#,
                r#"{"@timestamp":"2024-01-15T10:30:00Z","level":"WARN","logger_name":"app","message":"warn msg"}"#,
                r#"{"@timestamp":"2024-01-15T10:30:00Z","level":"ERROR","logger_name":"app","message":"error msg"}"#,
            ],
            |args| {
                args.min_level = Some(Level::Warn);
            },
        );
        assert!(!output.contains("debug msg"));
        assert!(!output.contains("info msg"));
        assert!(output.contains("warn msg"));
        assert!(output.contains("error msg"));
    }

    #[test]
    fn min_level_none_passes_all() {
        let output = run_with_input(
            &[
                r#"{"@timestamp":"2024-01-15T10:30:00Z","level":"DEBUG","logger_name":"app","message":"debug msg"}"#,
                r#"{"@timestamp":"2024-01-15T10:30:00Z","level":"ERROR","logger_name":"app","message":"error msg"}"#,
            ],
            |_args| {},
        );
        assert!(output.contains("debug msg"));
        assert!(output.contains("error msg"));
    }

    #[test]
    fn min_level_records_without_level_pass_through() {
        let output = run_with_input(
            &[r#"{"@timestamp":"2024-01-15T10:30:00Z","logger_name":"app","message":"no level"}"#],
            |args| {
                args.min_level = Some(Level::Warn);
            },
        );
        assert!(output.contains("no level"));
    }

    // --- Schema caching tests ---

    #[test]
    fn schema_cached_across_lines() {
        // First line triggers Logstash detection; second line has same format
        let output = run_with_input(
            &[
                r#"{"@timestamp":"2024-01-15T10:30:00Z","level":"INFO","logger_name":"app","message":"first"}"#,
                r#"{"@timestamp":"2024-01-15T10:31:00Z","level":"WARN","logger_name":"app","message":"second"}"#,
            ],
            |_args| {},
        );
        assert!(output.contains("first"));
        assert!(output.contains("second"));
        assert!(output.contains("INFO"));
        assert!(output.contains("WARN"));
    }

    #[test]
    fn forced_schema_used() {
        let output = run_with_input(
            &[
                r#"{"level":"info","msg":"logrus message","time":"2024-01-15T10:30:00Z","component":"web"}"#,
            ],
            |args| {
                args.schema = SchemaChoice::Logrus;
            },
        );
        assert!(output.contains("logrus message"));
        assert!(output.contains("web"));
    }

    // --- Non-JSON pass-through tests ---

    #[test]
    fn non_json_print_as_is() {
        let output = run_with_input(
            &[
                "plain text line",
                r#"{"@timestamp":"2024-01-15T10:30:00Z","level":"INFO","logger_name":"app","message":"json line"}"#,
                "another plain line",
            ],
            |_args| {},
        );
        assert!(output.contains("plain text line"));
        assert!(output.contains("json line"));
        assert!(output.contains("another plain line"));
    }

    #[test]
    fn non_json_skip() {
        let output = run_with_input(
            &[
                "plain text line",
                r#"{"@timestamp":"2024-01-15T10:30:00Z","level":"INFO","logger_name":"app","message":"json line"}"#,
                "another plain line",
            ],
            |args| {
                args.non_json = NonJsonMode::Skip;
            },
        );
        assert!(!output.contains("plain text line"));
        assert!(output.contains("json line"));
        assert!(!output.contains("another plain line"));
    }

    #[test]
    fn non_json_fail_returns_error() {
        let input = write_input(&["not json"]);
        let output = NamedTempFile::new().unwrap();
        let output_path = output.path().to_owned();

        let mut args = default_args();
        args.files = vec![input.path().to_path_buf()];
        args.output = Some(output_path);
        args.non_json = NonJsonMode::Fail;

        let result = run(args);
        assert!(result.is_err());
    }

    // --- Multiple file tests ---

    #[test]
    fn multiple_files_processed_sequentially() {
        let input1 = write_input(&[
            r#"{"@timestamp":"2024-01-15T10:30:00Z","level":"INFO","logger_name":"app","message":"from file 1"}"#,
        ]);
        let input2 = write_input(&[
            r#"{"@timestamp":"2024-01-15T10:31:00Z","level":"WARN","logger_name":"app","message":"from file 2"}"#,
        ]);
        let output = NamedTempFile::new().unwrap();
        let output_path = output.path().to_owned();

        let mut args = default_args();
        args.files = vec![input1.path().to_path_buf(), input2.path().to_path_buf()];
        args.output = Some(output_path.clone());

        run(args).unwrap();
        let contents = std::fs::read_to_string(&output_path).unwrap();
        assert!(contents.contains("from file 1"));
        assert!(contents.contains("from file 2"));

        // file 1 content should appear before file 2 content
        let pos1 = contents.find("from file 1").unwrap();
        let pos2 = contents.find("from file 2").unwrap();
        assert!(pos1 < pos2);
    }

    #[test]
    fn nonexistent_file_returns_error() {
        let mut args = default_args();
        args.files = vec![PathBuf::from("/nonexistent/file.log")];
        let result = run(args);
        assert!(result.is_err());
    }

    // --- Empty input test ---

    #[test]
    fn empty_input_produces_no_output() {
        let output = run_with_input(&[], |_args| {});
        assert!(output.is_empty());
    }

    // --- Raw JSON mode test ---

    #[test]
    fn raw_json_mode_passes_through() {
        let output = run_with_input(
            &[r#"{"level":"INFO","message":"test","extra":"data"}"#],
            |args| {
                args.raw_json = true;
            },
        );
        let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
        assert_eq!(parsed["level"], "INFO");
        assert_eq!(parsed["message"], "test");
        assert_eq!(parsed["extra"], "data");
    }

    // --- Compact mode test ---

    #[test]
    fn compact_mode_extras_on_same_line() {
        let output = run_with_input(
            &[
                r#"{"@timestamp":"2024-01-15T10:30:00Z","level":"INFO","logger_name":"app","message":"test","extra_field":"value"}"#,
            ],
            |args| {
                args.compact = true;
            },
        );
        // Each output record should be a single line (compact mode)
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("extra_field=value"));
    }
}
