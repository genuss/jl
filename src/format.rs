use std::collections::HashSet;

use owo_colors::{OwoColorize, Style};
use serde_json::Value;

use crate::cli::Args;
use crate::color::ColorConfig;
use crate::record::LogRecord;

/// A parsed token from a format template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatToken {
    /// Literal text to output as-is.
    Literal(String),
    /// A canonical field placeholder: level, timestamp, logger, message.
    Field(CanonicalField),
    /// A custom (non-canonical) field placeholder by name.
    CustomField(String),
}

/// Canonical fields that can appear in format templates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanonicalField {
    Level,
    Timestamp,
    Logger,
    Message,
}

/// Parse a format template string into a sequence of tokens.
///
/// Placeholders use `{field_name}` syntax. Known canonical fields are mapped to
/// `CanonicalField` variants; everything else becomes `CustomField`.
/// Literal `{` and `}` can be escaped by doubling: `{{` and `}}`.
pub fn parse_template(template: &str) -> Vec<FormatToken> {
    let mut tokens = Vec::new();
    let mut chars = template.chars().peekable();
    let mut literal = String::new();

    while let Some(ch) = chars.next() {
        if ch == '{' {
            if chars.peek() == Some(&'{') {
                // Escaped opening brace
                chars.next();
                literal.push('{');
            } else {
                // Start of a placeholder
                if !literal.is_empty() {
                    tokens.push(FormatToken::Literal(std::mem::take(&mut literal)));
                }
                let mut field_name = String::new();
                for inner_ch in chars.by_ref() {
                    if inner_ch == '}' {
                        break;
                    }
                    field_name.push(inner_ch);
                }
                let token = match field_name.as_str() {
                    "level" => FormatToken::Field(CanonicalField::Level),
                    "timestamp" => FormatToken::Field(CanonicalField::Timestamp),
                    "logger" => FormatToken::Field(CanonicalField::Logger),
                    "message" => FormatToken::Field(CanonicalField::Message),
                    _ => FormatToken::CustomField(field_name),
                };
                tokens.push(token);
            }
        } else if ch == '}' {
            if chars.peek() == Some(&'}') {
                // Escaped closing brace
                chars.next();
                literal.push('}');
            } else {
                literal.push(ch);
            }
        } else {
            literal.push(ch);
        }
    }

    if !literal.is_empty() {
        tokens.push(FormatToken::Literal(literal));
    }

    tokens
}

/// Render a log record using the given format tokens, color config, and CLI args.
///
/// Handles:
/// - Field substitution from the record
/// - Color styling for the level field
/// - `--raw-json` mode (outputs the original JSON)
/// - `--add-fields` / `--omit-fields` for controlling extra field output
/// - `--compact` mode (extras on same line vs separate lines)
/// - Stack trace appending after the main line
pub fn render(
    record: &LogRecord,
    tokens: &[FormatToken],
    color: &ColorConfig,
    args: &Args,
) -> String {
    // --raw-json: just output the raw JSON
    if args.raw_json {
        return record.raw.to_string();
    }

    let omit_fields = parse_field_list(args.omit_fields.as_deref());
    let add_fields = parse_field_list(args.add_fields.as_deref());

    // Collect custom field names referenced in the template so we can exclude
    // them from the extras section (they're already shown inline).
    let template_custom_fields: HashSet<String> = tokens
        .iter()
        .filter_map(|t| match t {
            FormatToken::CustomField(name) => Some(name.clone()),
            _ => None,
        })
        .collect();

    // Build the main formatted line from the template
    let mut line = String::new();
    for token in tokens {
        match token {
            FormatToken::Literal(s) => line.push_str(s),
            FormatToken::Field(field) => {
                let value = match field {
                    CanonicalField::Level => match &record.level {
                        Some(level) => color.style_level(level),
                        None => String::new(),
                    },
                    CanonicalField::Timestamp => record.timestamp.clone().unwrap_or_default(),
                    CanonicalField::Logger => record.logger.clone().unwrap_or_default(),
                    CanonicalField::Message => record.message.clone().unwrap_or_default(),
                };
                line.push_str(&value);
            }
            FormatToken::CustomField(name) => {
                let value = record
                    .extras
                    .get(name)
                    .map(format_extra_value)
                    .unwrap_or_default();
                line.push_str(&value);
            }
        }
    }

    // Determine which extras to include
    let extras = collect_extras(record, &add_fields, &omit_fields, &template_custom_fields);

    // Append extras
    if !extras.is_empty() {
        if args.compact {
            // Compact mode: extras on the same line
            let extras_str: Vec<String> = extras
                .iter()
                .map(|(k, v)| format!("{k}={}", format_extra_value(v)))
                .collect();
            line.push(' ');
            line.push_str(&extras_str.join(" "));
        } else {
            // Normal mode: extras on separate lines, indented
            for (k, v) in &extras {
                line.push('\n');
                line.push_str(&format!("  {k}: {}", format_extra_value(v)));
            }
        }
    }

    // Append stack trace if present and not omitted
    if let Some(ref st) = record.stack_trace
        && !omit_fields.contains("stack_trace")
    {
        append_stack_trace(&mut line, st, color);
    }

    line
}

/// Append a stack trace to the output line with indentation and optional dimming.
///
/// Each line of the stack trace is indented with 4 spaces. When color is enabled,
/// the entire stack trace is rendered in a dimmed style for visual distinction
/// from the main log line.
fn append_stack_trace(line: &mut String, stack_trace: &str, color: &ColorConfig) {
    let dim_style = if color.enabled {
        Style::new().dimmed()
    } else {
        Style::new()
    };

    for trace_line in stack_trace.lines() {
        line.push('\n');
        let indented = format!("    {trace_line}");
        if color.enabled {
            line.push_str(&format!("{}", indented.style(dim_style)));
        } else {
            line.push_str(&indented);
        }
    }
}

/// Collect extra fields to display, respecting --add-fields, --omit-fields,
/// and excluding fields already referenced in the template.
///
/// If `add_fields` is non-empty, only those extras are included (allowlist).
/// If `omit_fields` is non-empty, those extras are excluded (denylist).
/// Fields already shown via template custom field placeholders are excluded.
/// If both add/omit are empty, all non-template extras are included.
fn collect_extras<'a>(
    record: &'a LogRecord,
    add_fields: &HashSet<String>,
    omit_fields: &HashSet<String>,
    template_fields: &HashSet<String>,
) -> Vec<(&'a String, &'a Value)> {
    record
        .extras
        .iter()
        .filter(|(k, _)| {
            // Exclude fields already rendered inline via the template
            if template_fields.contains(k.as_str()) {
                return false;
            }
            if !add_fields.is_empty() {
                add_fields.contains(k.as_str())
            } else if !omit_fields.is_empty() {
                !omit_fields.contains(k.as_str())
            } else {
                true
            }
        })
        .collect()
}

/// Parse a comma-separated field list into a set of field names.
fn parse_field_list(list: Option<&str>) -> HashSet<String> {
    match list {
        Some(s) => s
            .split(',')
            .map(|f| f.trim().to_string())
            .filter(|f| !f.is_empty())
            .collect(),
        None => HashSet::new(),
    }
}

/// Format a JSON value for display as an extra field value.
fn format_extra_value(val: &Value) -> String {
    match val {
        Value::String(s) => s.clone(),
        _ => val.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{ColorMode, NonJsonMode, SchemaChoice};
    use crate::level::Level;
    use crate::record::LogRecord;
    use serde_json::json;
    use std::collections::BTreeMap;

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

    fn make_record(
        level: Option<Level>,
        timestamp: Option<&str>,
        logger: Option<&str>,
        message: Option<&str>,
    ) -> LogRecord {
        LogRecord {
            level,
            timestamp: timestamp.map(String::from),
            logger: logger.map(String::from),
            message: message.map(String::from),
            stack_trace: None,
            extras: BTreeMap::new(),
            raw: json!({}),
        }
    }

    // --- Template parsing tests ---

    #[test]
    fn parse_default_template() {
        let tokens = parse_template("{timestamp} {level} [{logger}] {message}");
        assert_eq!(
            tokens,
            vec![
                FormatToken::Field(CanonicalField::Timestamp),
                FormatToken::Literal(" ".to_string()),
                FormatToken::Field(CanonicalField::Level),
                FormatToken::Literal(" [".to_string()),
                FormatToken::Field(CanonicalField::Logger),
                FormatToken::Literal("] ".to_string()),
                FormatToken::Field(CanonicalField::Message),
            ]
        );
    }

    #[test]
    fn parse_custom_field() {
        let tokens = parse_template("{level} {host}: {message}");
        assert_eq!(
            tokens,
            vec![
                FormatToken::Field(CanonicalField::Level),
                FormatToken::Literal(" ".to_string()),
                FormatToken::CustomField("host".to_string()),
                FormatToken::Literal(": ".to_string()),
                FormatToken::Field(CanonicalField::Message),
            ]
        );
    }

    #[test]
    fn parse_literals_only() {
        let tokens = parse_template("no fields here");
        assert_eq!(
            tokens,
            vec![FormatToken::Literal("no fields here".to_string())]
        );
    }

    #[test]
    fn parse_adjacent_fields() {
        let tokens = parse_template("{level}{message}");
        assert_eq!(
            tokens,
            vec![
                FormatToken::Field(CanonicalField::Level),
                FormatToken::Field(CanonicalField::Message),
            ]
        );
    }

    #[test]
    fn parse_escaped_braces() {
        let tokens = parse_template("{{literal braces}}");
        assert_eq!(
            tokens,
            vec![FormatToken::Literal("{literal braces}".to_string())]
        );
    }

    #[test]
    fn parse_empty_template() {
        let tokens = parse_template("");
        assert!(tokens.is_empty());
    }

    #[test]
    fn parse_field_at_start_and_end() {
        let tokens = parse_template("{level} test {message}");
        assert_eq!(
            tokens,
            vec![
                FormatToken::Field(CanonicalField::Level),
                FormatToken::Literal(" test ".to_string()),
                FormatToken::Field(CanonicalField::Message),
            ]
        );
    }

    // --- Rendering tests ---

    #[test]
    fn render_basic_no_color() {
        let record = make_record(
            Some(Level::Info),
            Some("2024-01-15T10:30:00.000Z"),
            Some("com.example"),
            Some("hello world"),
        );
        let tokens = parse_template("{timestamp} {level} [{logger}] {message}");
        let color = ColorConfig::with_enabled(false);
        let args = default_args();
        let output = render(&record, &tokens, &color, &args);
        assert_eq!(
            output,
            "2024-01-15T10:30:00.000Z INFO [com.example] hello world"
        );
    }

    #[test]
    fn render_with_color() {
        let record = make_record(Some(Level::Error), None, None, Some("fail"));
        let tokens = parse_template("{level}: {message}");
        let color = ColorConfig::with_enabled(true);
        let args = default_args();
        let output = render(&record, &tokens, &color, &args);
        // Should contain ANSI codes for ERROR level (red)
        assert!(output.contains("\x1b["));
        assert!(output.contains("ERROR"));
        assert!(output.contains("fail"));
    }

    #[test]
    fn render_missing_fields() {
        let record = make_record(None, None, None, Some("just a message"));
        let tokens = parse_template("{timestamp} {level} [{logger}] {message}");
        let color = ColorConfig::with_enabled(false);
        let args = default_args();
        let output = render(&record, &tokens, &color, &args);
        assert_eq!(output, "  [] just a message");
    }

    #[test]
    fn render_with_extras_normal_mode() {
        let mut record = make_record(Some(Level::Info), None, None, Some("test"));
        record.extras.insert("host".to_string(), json!("server1"));
        record.extras.insert("pid".to_string(), json!(1234));
        let tokens = parse_template("{level}: {message}");
        let color = ColorConfig::with_enabled(false);
        let args = default_args();
        let output = render(&record, &tokens, &color, &args);
        assert!(output.contains("INFO: test"));
        assert!(output.contains("\n  host: server1"));
        assert!(output.contains("\n  pid: 1234"));
    }

    #[test]
    fn render_with_extras_compact_mode() {
        let mut record = make_record(Some(Level::Info), None, None, Some("test"));
        record.extras.insert("host".to_string(), json!("server1"));
        record.extras.insert("pid".to_string(), json!(1234));
        let tokens = parse_template("{level}: {message}");
        let color = ColorConfig::with_enabled(false);
        let mut args = default_args();
        args.compact = true;
        let output = render(&record, &tokens, &color, &args);
        assert!(output.contains("INFO: test"));
        // Compact: extras on same line, not on separate lines
        assert!(!output.contains('\n'));
        assert!(output.contains("host=server1"));
        assert!(output.contains("pid=1234"));
    }

    #[test]
    fn render_with_omit_fields() {
        let mut record = make_record(Some(Level::Info), None, None, Some("test"));
        record.extras.insert("host".to_string(), json!("server1"));
        record.extras.insert("pid".to_string(), json!(1234));
        record.extras.insert("secret".to_string(), json!("hidden"));
        let tokens = parse_template("{level}: {message}");
        let color = ColorConfig::with_enabled(false);
        let mut args = default_args();
        args.omit_fields = Some("secret,pid".to_string());
        args.compact = true;
        let output = render(&record, &tokens, &color, &args);
        assert!(output.contains("host=server1"));
        assert!(!output.contains("secret"));
        assert!(!output.contains("pid"));
    }

    #[test]
    fn render_with_add_fields() {
        let mut record = make_record(Some(Level::Info), None, None, Some("test"));
        record.extras.insert("host".to_string(), json!("server1"));
        record.extras.insert("pid".to_string(), json!(1234));
        record.extras.insert("region".to_string(), json!("us-east"));
        let tokens = parse_template("{level}: {message}");
        let color = ColorConfig::with_enabled(false);
        let mut args = default_args();
        args.add_fields = Some("host".to_string());
        args.compact = true;
        let output = render(&record, &tokens, &color, &args);
        // Only host should be included
        assert!(output.contains("host=server1"));
        assert!(!output.contains("pid"));
        assert!(!output.contains("region"));
    }

    #[test]
    fn render_raw_json_mode() {
        let raw = json!({
            "level": "INFO",
            "message": "hello",
            "extra": "data"
        });
        let record = LogRecord {
            level: Some(Level::Info),
            timestamp: None,
            logger: None,
            message: Some("hello".to_string()),
            stack_trace: None,
            extras: BTreeMap::new(),
            raw: raw.clone(),
        };
        let tokens = parse_template("{level}: {message}");
        let color = ColorConfig::with_enabled(false);
        let mut args = default_args();
        args.raw_json = true;
        let output = render(&record, &tokens, &color, &args);
        // Should be the raw JSON, not the formatted template
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed, raw);
    }

    #[test]
    fn render_with_stack_trace() {
        let mut record = make_record(Some(Level::Error), None, None, Some("crash"));
        record.stack_trace =
            Some("java.lang.NullPointerException\n\tat Foo.bar(Foo.java:42)".to_string());
        let tokens = parse_template("{level}: {message}");
        let color = ColorConfig::with_enabled(false);
        let args = default_args();
        let output = render(&record, &tokens, &color, &args);
        assert!(output.contains("ERROR: crash"));
        assert!(output.contains("\n    java.lang.NullPointerException"));
        assert!(output.contains("\n    \tat Foo.bar(Foo.java:42)"));
    }

    #[test]
    fn render_stack_trace_omitted() {
        let mut record = make_record(Some(Level::Error), None, None, Some("crash"));
        record.stack_trace = Some("java.lang.NullPointerException".to_string());
        let tokens = parse_template("{level}: {message}");
        let color = ColorConfig::with_enabled(false);
        let mut args = default_args();
        args.omit_fields = Some("stack_trace".to_string());
        let output = render(&record, &tokens, &color, &args);
        assert!(output.contains("ERROR: crash"));
        assert!(!output.contains("NullPointerException"));
    }

    #[test]
    fn render_custom_field_in_template() {
        let mut record = make_record(Some(Level::Info), None, None, Some("test"));
        record.extras.insert("host".to_string(), json!("server1"));
        let tokens = parse_template("{level} [{host}] {message}");
        let color = ColorConfig::with_enabled(false);
        let args = default_args();
        let output = render(&record, &tokens, &color, &args);
        assert_eq!(output, "INFO [server1] test");
    }

    #[test]
    fn render_custom_field_missing_from_extras() {
        let record = make_record(Some(Level::Info), None, None, Some("test"));
        let tokens = parse_template("{level} [{host}] {message}");
        let color = ColorConfig::with_enabled(false);
        let args = default_args();
        let output = render(&record, &tokens, &color, &args);
        assert_eq!(output, "INFO [] test");
    }

    #[test]
    fn render_no_extras_no_stack_trace() {
        let record = make_record(
            Some(Level::Debug),
            Some("2024-01-15T10:30:00.000Z"),
            Some("app"),
            Some("started"),
        );
        let tokens = parse_template("{timestamp} {level} [{logger}] {message}");
        let color = ColorConfig::with_enabled(false);
        let args = default_args();
        let output = render(&record, &tokens, &color, &args);
        assert_eq!(output, "2024-01-15T10:30:00.000Z DEBUG [app] started");
        assert!(!output.contains('\n'));
    }

    // --- parse_field_list tests ---

    #[test]
    fn parse_field_list_none() {
        let result = parse_field_list(None);
        assert!(result.is_empty());
    }

    #[test]
    fn parse_field_list_single() {
        let result = parse_field_list(Some("host"));
        assert_eq!(result.len(), 1);
        assert!(result.contains("host"));
    }

    #[test]
    fn parse_field_list_multiple() {
        let result = parse_field_list(Some("host,pid,region"));
        assert_eq!(result.len(), 3);
        assert!(result.contains("host"));
        assert!(result.contains("pid"));
        assert!(result.contains("region"));
    }

    #[test]
    fn parse_field_list_with_spaces() {
        let result = parse_field_list(Some("host , pid , region"));
        assert_eq!(result.len(), 3);
        assert!(result.contains("host"));
        assert!(result.contains("pid"));
        assert!(result.contains("region"));
    }

    // --- format_extra_value tests ---

    #[test]
    fn format_extra_value_string() {
        assert_eq!(format_extra_value(&json!("hello")), "hello");
    }

    #[test]
    fn format_extra_value_number() {
        assert_eq!(format_extra_value(&json!(42)), "42");
    }

    #[test]
    fn format_extra_value_bool() {
        assert_eq!(format_extra_value(&json!(true)), "true");
    }

    #[test]
    fn format_extra_value_null() {
        assert_eq!(format_extra_value(&json!(null)), "null");
    }

    #[test]
    fn format_extra_value_object() {
        let val = json!({"key": "val"});
        let result = format_extra_value(&val);
        assert!(result.contains("key"));
        assert!(result.contains("val"));
    }

    // --- Stack trace formatting tests ---

    #[test]
    fn stack_trace_multiline_indentation() {
        let mut record = make_record(Some(Level::Error), None, None, Some("crash"));
        record.stack_trace = Some(
            "java.lang.NullPointerException: null\n\
             \tat com.example.Service.process(Service.java:42)\n\
             \tat com.example.Controller.handle(Controller.java:15)\n\
             Caused by: java.io.IOException: connection reset\n\
             \tat com.example.Client.connect(Client.java:88)"
                .to_string(),
        );
        let tokens = parse_template("{level}: {message}");
        let color = ColorConfig::with_enabled(false);
        let args = default_args();
        let output = render(&record, &tokens, &color, &args);

        let lines: Vec<&str> = output.lines().collect();
        // First line is the log message
        assert_eq!(lines[0], "ERROR: crash");
        // Each stack trace line should be indented with 4 spaces
        assert_eq!(lines[1], "    java.lang.NullPointerException: null");
        assert_eq!(
            lines[2],
            "    \tat com.example.Service.process(Service.java:42)"
        );
        assert_eq!(
            lines[3],
            "    \tat com.example.Controller.handle(Controller.java:15)"
        );
        assert_eq!(
            lines[4],
            "    Caused by: java.io.IOException: connection reset"
        );
        assert_eq!(
            lines[5],
            "    \tat com.example.Client.connect(Client.java:88)"
        );
        assert_eq!(lines.len(), 6);
    }

    #[test]
    fn stack_trace_single_line() {
        let mut record = make_record(Some(Level::Error), None, None, Some("oops"));
        record.stack_trace = Some("Error: something went wrong".to_string());
        let tokens = parse_template("{level}: {message}");
        let color = ColorConfig::with_enabled(false);
        let args = default_args();
        let output = render(&record, &tokens, &color, &args);

        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines[0], "ERROR: oops");
        assert_eq!(lines[1], "    Error: something went wrong");
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn stack_trace_dimmed_with_color() {
        let mut record = make_record(Some(Level::Error), None, None, Some("fail"));
        record.stack_trace = Some("StackLine1\nStackLine2".to_string());
        let tokens = parse_template("{level}: {message}");
        let color = ColorConfig::with_enabled(true);
        let args = default_args();
        let output = render(&record, &tokens, &color, &args);

        // With color enabled, stack trace lines should contain ANSI dimmed code (\x1b[2m)
        assert!(output.contains("\x1b[2m"));
        assert!(output.contains("StackLine1"));
        assert!(output.contains("StackLine2"));
    }

    #[test]
    fn stack_trace_no_ansi_without_color() {
        let mut record = make_record(Some(Level::Error), None, None, Some("fail"));
        record.stack_trace = Some("StackLine1\nStackLine2".to_string());
        let tokens = parse_template("{level}: {message}");
        let color = ColorConfig::with_enabled(false);
        let args = default_args();
        let output = render(&record, &tokens, &color, &args);

        // Without color, no ANSI codes should be present in the stack trace
        assert!(!output.contains("\x1b["));
        assert!(output.contains("    StackLine1"));
        assert!(output.contains("    StackLine2"));
    }

    #[test]
    fn stack_trace_with_extras_and_compact() {
        let mut record = make_record(Some(Level::Error), None, None, Some("fail"));
        record.stack_trace = Some("Error\n\tat line 1".to_string());
        record.extras.insert("host".to_string(), json!("server1"));
        let tokens = parse_template("{level}: {message}");
        let color = ColorConfig::with_enabled(false);
        let mut args = default_args();
        args.compact = true;
        let output = render(&record, &tokens, &color, &args);

        // Extras should be on the same line (compact), stack trace on new lines
        let lines: Vec<&str> = output.lines().collect();
        assert!(lines[0].contains("ERROR: fail"));
        assert!(lines[0].contains("host=server1"));
        assert_eq!(lines[1], "    Error");
        assert_eq!(lines[2], "    \tat line 1");
    }

    #[test]
    fn stack_trace_empty_string() {
        let mut record = make_record(Some(Level::Error), None, None, Some("fail"));
        record.stack_trace = Some("".to_string());
        let tokens = parse_template("{level}: {message}");
        let color = ColorConfig::with_enabled(false);
        let args = default_args();
        let output = render(&record, &tokens, &color, &args);

        // Empty stack trace produces no additional lines (Rust's .lines() on "" yields nothing)
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], "ERROR: fail");
    }
}
