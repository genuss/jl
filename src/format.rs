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

/// Pre-computed rendering context to avoid redundant work per line.
///
/// Created once before the processing loop and reused across all records.
pub struct RenderContext {
    pub omit_fields: HashSet<String>,
    pub add_fields: HashSet<String>,
    pub template_custom_fields: HashSet<String>,
}

impl RenderContext {
    /// Build a render context from CLI args and parsed format tokens.
    pub fn new(args: &Args, tokens: &[FormatToken]) -> Self {
        let omit_fields = parse_field_list(args.omit_fields.as_deref());
        let add_fields = parse_field_list(args.add_fields.as_deref());
        let template_custom_fields = tokens
            .iter()
            .filter_map(|t| match t {
                FormatToken::CustomField(name) => Some(name.clone()),
                _ => None,
            })
            .collect();
        Self {
            omit_fields,
            add_fields,
            template_custom_fields,
        }
    }
}

/// Render a log record using the given format tokens, color config, and CLI args.
///
/// Handles:
/// - Field substitution from the record
/// - Color styling for the level field
/// - `--raw-json` mode (outputs the original JSON)
/// - `--add-fields` / `--omit-fields` for controlling extra field output
/// - `--expanded` mode (extras on separate lines vs same line)
/// - Stack trace appending after the main line
pub fn render(
    record: &LogRecord,
    tokens: &[FormatToken],
    color: &ColorConfig,
    args: &Args,
    ctx: &RenderContext,
) -> String {
    // --raw-json: just output the raw JSON
    if args.raw_json {
        return record.raw.to_string();
    }

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
                    CanonicalField::Timestamp => {
                        sanitize_control_chars(&record.timestamp.clone().unwrap_or_default())
                    }
                    CanonicalField::Logger => {
                        let raw = record.logger.clone().unwrap_or_default();
                        let formatted = match args.logger_format {
                            crate::cli::LoggerFormat::ShortDots => shorten_logger_dots(&raw),
                            crate::cli::LoggerFormat::AsIs => raw,
                        };
                        let truncated =
                            truncate_logger_left(&formatted, args.logger_length);
                        sanitize_control_chars(&truncated)
                    }
                    CanonicalField::Message => {
                        sanitize_control_chars(&record.message.clone().unwrap_or_default())
                    }
                };
                line.push_str(&value);
            }
            FormatToken::CustomField(name) => {
                let value = record
                    .extras
                    .get(name)
                    .map(|v| sanitize_control_chars(&format_extra_value(v)))
                    .unwrap_or_default();
                line.push_str(&value);
            }
        }
    }

    // Determine which extras to include
    let extras = collect_extras(
        record,
        &ctx.add_fields,
        &ctx.omit_fields,
        &ctx.template_custom_fields,
    );

    // Append extras
    if !extras.is_empty() {
        if args.expanded {
            // Expanded mode: extras on separate lines, indented
            for (k, v) in &extras {
                line.push('\n');
                line.push_str(&format!(
                    "  {}: {}",
                    color.style_extra_key(&sanitize_control_chars(k)),
                    color.style_extra_value(&sanitize_control_chars(&format_extra_value(v)))
                ));
            }
        } else {
            // Compact mode (default): extras on the same line
            let extras_str: Vec<String> = extras
                .iter()
                .map(|(k, v)| {
                    format!(
                        "{}={}",
                        color.style_extra_key(&sanitize_control_chars(k)),
                        color.style_extra_value(&sanitize_control_chars(&format_extra_value(v)))
                    )
                })
                .collect();
            line.push(' ');
            line.push_str(&extras_str.join(" "));
        }
    }

    // Append stack trace if present and not omitted
    if let Some(ref st) = record.stack_trace
        && !ctx.omit_fields.contains("stack_trace")
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

    let sanitized = sanitize_control_chars(stack_trace);
    for trace_line in sanitized.lines() {
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
/// The two flags are mutually exclusive (enforced by CLI validation).
/// Fields already shown via template custom field placeholders are excluded.
/// If neither flag is set, no extras are included (opt-in model).
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
                false
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

/// Abbreviate dot-separated logger name segments, keeping only the first character
/// of each segment except the last.
///
/// For example: `com.example.service.MyHandler` → `c.e.s.MyHandler`
///
/// Single-segment names and empty strings are returned unchanged.
pub fn shorten_logger_dots(name: &str) -> String {
    if name.is_empty() {
        return String::new();
    }
    let segments: Vec<&str> = name.split('.').collect();
    if segments.len() <= 1 {
        return name.to_string();
    }
    let mut parts: Vec<String> = segments[..segments.len() - 1]
        .iter()
        .map(|s| {
            s.chars()
                .next()
                .map(|c| c.to_string())
                .unwrap_or_default()
        })
        .collect();
    parts.push(segments[segments.len() - 1].to_string());
    parts.join(".")
}

/// Truncate a logger name from the left when it exceeds `max_len`.
///
/// First tries to strip leftmost dot-separated segments one at a time until the name fits.
/// If the name still exceeds `max_len` after stripping all removable segments (or has no dots),
/// hard-truncates from the left to exactly `max_len` characters.
///
/// A `max_len` of 0 means no truncation (unlimited).
///
/// Examples:
/// - `truncate_logger_left("c.e.s.MyHandler", 15)` → `"c.e.s.MyHandler"` (fits)
/// - `truncate_logger_left("c.e.s.MyHandler", 13)` → `"e.s.MyHandler"` (stripped "c.")
/// - `truncate_logger_left("VeryLongName", 4)` → `"Name"` (hard-truncated)
pub fn truncate_logger_left(name: &str, max_len: usize) -> String {
    if max_len == 0 || name.chars().count() <= max_len {
        return name.to_string();
    }

    // Try stripping leftmost dot-segments one at a time
    let mut remaining = name;
    while remaining.chars().count() > max_len {
        if let Some(dot_pos) = remaining.find('.') {
            let after_dot = &remaining[dot_pos + 1..];
            if after_dot.is_empty() {
                break;
            }
            remaining = after_dot;
        } else {
            break;
        }
    }

    // If still too long, hard-truncate from the left
    let char_count = remaining.chars().count();
    if char_count > max_len {
        remaining.chars().skip(char_count - max_len).collect()
    } else {
        remaining.to_string()
    }
}

/// Strip terminal control characters from a string to prevent escape sequence injection.
///
/// Removes C0 control characters (0x00-0x1F) except TAB (0x09) and newline (0x0A),
/// and C1 control characters (0x80-0x9F). This prevents hostile log data from
/// injecting ANSI escape sequences (CSI/OSC/DCS) into the terminal.
pub fn sanitize_control_chars(s: &str) -> String {
    s.chars()
        .filter(|&c| {
            // Allow TAB and newline, strip all other C0 and all C1 control chars
            if c == '\t' || c == '\n' {
                true
            } else {
                !c.is_control()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{ColorMode, LoggerFormat, NonJsonMode, SchemaChoice, TsFormat};
    use crate::level::Level;
    use crate::record::LogRecord;
    use serde_json::json;
    use std::collections::BTreeMap;

    fn test_render(
        record: &LogRecord,
        tokens: &[FormatToken],
        color: &ColorConfig,
        args: &Args,
    ) -> String {
        let ctx = RenderContext::new(args, tokens);
        render(record, tokens, color, args, &ctx)
    }

    fn default_args() -> Args {
        Args {
            format: "{timestamp} {level} [{logger}] {message}".to_string(),
            add_fields: None,
            omit_fields: None,
            color: ColorMode::Never,
            non_json: NonJsonMode::PrintAsIs,
            schema: SchemaChoice::Auto,
            logger_format: LoggerFormat::AsIs,
            logger_length: 0,
            ts_format: TsFormat::Full,
            min_level: None,
            raw_json: false,
            expanded: false,
            key_color: crate::cli::CliColor::Magenta,
            value_color: crate::cli::CliColor::Cyan,
            tz: "utc".to_string(),
            follow: false,
            output: None,
            completions: None,
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
            Some("2024-01-15T10:30:00.000"),
            Some("com.example"),
            Some("hello world"),
        );
        let tokens = parse_template("{timestamp} {level} [{logger}] {message}");
        let color = ColorConfig::with_enabled(false);
        let args = default_args();
        let output = test_render(&record, &tokens, &color, &args);
        assert_eq!(
            output,
            "2024-01-15T10:30:00.000 INFO [com.example] hello world"
        );
    }

    #[test]
    fn render_with_color() {
        let record = make_record(Some(Level::Error), None, None, Some("fail"));
        let tokens = parse_template("{level}: {message}");
        let color = ColorConfig::with_enabled(true);
        let args = default_args();
        let output = test_render(&record, &tokens, &color, &args);
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
        let output = test_render(&record, &tokens, &color, &args);
        assert_eq!(output, "  [] just a message");
    }

    #[test]
    fn render_with_extras_expanded_mode() {
        let mut record = make_record(Some(Level::Info), None, None, Some("test"));
        record.extras.insert("host".to_string(), json!("server1"));
        record.extras.insert("pid".to_string(), json!(1234));
        let tokens = parse_template("{level}: {message}");
        let color = ColorConfig::with_enabled(false);
        let mut args = default_args();
        args.expanded = true;
        args.add_fields = Some("host,pid".to_string());
        let output = test_render(&record, &tokens, &color, &args);
        assert!(output.contains("INFO: test"));
        assert!(output.contains("\n  host: server1"));
        assert!(output.contains("\n  pid: 1234"));
    }

    #[test]
    fn render_no_extras_by_default() {
        let mut record = make_record(Some(Level::Info), None, None, Some("test"));
        record.extras.insert("host".to_string(), json!("server1"));
        record.extras.insert("pid".to_string(), json!(1234));
        let tokens = parse_template("{level}: {message}");
        let color = ColorConfig::with_enabled(false);
        let args = default_args();
        let output = test_render(&record, &tokens, &color, &args);
        assert_eq!(output, "INFO: test");
    }

    #[test]
    fn render_with_extras_compact_mode_default() {
        let mut record = make_record(Some(Level::Info), None, None, Some("test"));
        record.extras.insert("host".to_string(), json!("server1"));
        record.extras.insert("pid".to_string(), json!(1234));
        let tokens = parse_template("{level}: {message}");
        let color = ColorConfig::with_enabled(false);
        let mut args = default_args();
        args.add_fields = Some("host,pid".to_string());
        let output = test_render(&record, &tokens, &color, &args);
        assert!(output.contains("INFO: test"));
        // Compact is now the default: extras on same line, not on separate lines
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
        let output = test_render(&record, &tokens, &color, &args);
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
        let output = test_render(&record, &tokens, &color, &args);
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
        let output = test_render(&record, &tokens, &color, &args);
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
        let output = test_render(&record, &tokens, &color, &args);
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
        let output = test_render(&record, &tokens, &color, &args);
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
        let output = test_render(&record, &tokens, &color, &args);
        assert_eq!(output, "INFO [server1] test");
    }

    #[test]
    fn render_custom_field_missing_from_extras() {
        let record = make_record(Some(Level::Info), None, None, Some("test"));
        let tokens = parse_template("{level} [{host}] {message}");
        let color = ColorConfig::with_enabled(false);
        let args = default_args();
        let output = test_render(&record, &tokens, &color, &args);
        assert_eq!(output, "INFO [] test");
    }

    #[test]
    fn render_no_extras_no_stack_trace() {
        let record = make_record(
            Some(Level::Debug),
            Some("2024-01-15T10:30:00.000"),
            Some("app"),
            Some("started"),
        );
        let tokens = parse_template("{timestamp} {level} [{logger}] {message}");
        let color = ColorConfig::with_enabled(false);
        let args = default_args();
        let output = test_render(&record, &tokens, &color, &args);
        assert_eq!(output, "2024-01-15T10:30:00.000 DEBUG [app] started");
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
        let output = test_render(&record, &tokens, &color, &args);

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
        let output = test_render(&record, &tokens, &color, &args);

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
        let output = test_render(&record, &tokens, &color, &args);

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
        let output = test_render(&record, &tokens, &color, &args);

        // Without color, no ANSI codes should be present in the stack trace
        assert!(!output.contains("\x1b["));
        assert!(output.contains("    StackLine1"));
        assert!(output.contains("    StackLine2"));
    }

    #[test]
    fn stack_trace_with_extras_and_compact_default() {
        let mut record = make_record(Some(Level::Error), None, None, Some("fail"));
        record.stack_trace = Some("Error\n\tat line 1".to_string());
        record.extras.insert("host".to_string(), json!("server1"));
        let tokens = parse_template("{level}: {message}");
        let color = ColorConfig::with_enabled(false);
        let mut args = default_args();
        args.add_fields = Some("host".to_string());
        let output = test_render(&record, &tokens, &color, &args);

        // Extras should be on the same line (compact is default), stack trace on new lines
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
        let output = test_render(&record, &tokens, &color, &args);

        // Empty stack trace produces no additional lines (Rust's .lines() on "" yields nothing)
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], "ERROR: fail");
    }

    // --- Control character sanitization tests ---

    #[test]
    fn sanitize_strips_escape_sequences() {
        assert_eq!(
            sanitize_control_chars("hello\x1b[31mRED\x1b[0m world"),
            "hello[31mRED[0m world"
        );
    }

    #[test]
    fn sanitize_preserves_tabs() {
        assert_eq!(sanitize_control_chars("key\tvalue"), "key\tvalue");
    }

    #[test]
    fn sanitize_preserves_newlines() {
        assert_eq!(sanitize_control_chars("line1\nline2"), "line1\nline2");
    }

    #[test]
    fn sanitize_strips_null_bytes() {
        assert_eq!(sanitize_control_chars("hel\x00lo"), "hello");
    }

    #[test]
    fn sanitize_strips_c1_controls() {
        // C1 control range: 0x80-0x9F
        assert_eq!(sanitize_control_chars("test\u{0090}data"), "testdata");
        assert_eq!(sanitize_control_chars("test\u{009B}data"), "testdata");
    }

    #[test]
    fn sanitize_leaves_normal_text() {
        assert_eq!(sanitize_control_chars("normal text 123"), "normal text 123");
    }

    #[test]
    fn sanitize_strips_bell_and_backspace() {
        assert_eq!(sanitize_control_chars("a\x07b\x08c"), "abc");
    }

    #[test]
    fn render_sanitizes_message_with_escape_codes() {
        let record = make_record(
            Some(Level::Info),
            None,
            None,
            Some("evil\x1b[31m red text\x1b[0m"),
        );
        let tokens = parse_template("{level}: {message}");
        let color = ColorConfig::with_enabled(false);
        let args = default_args();
        let output = test_render(&record, &tokens, &color, &args);
        // ESC bytes should be stripped
        assert!(!output.contains('\x1b'));
        assert!(output.contains("evil[31m red text[0m"));
    }

    #[test]
    fn render_sanitizes_extras_with_escape_codes() {
        let mut record = make_record(Some(Level::Info), None, None, Some("test"));
        record
            .extras
            .insert("bad".to_string(), json!("\x1b[31mred\x1b[0m"));
        let tokens = parse_template("{level}: {message}");
        let color = ColorConfig::with_enabled(false);
        let mut args = default_args();
        args.add_fields = Some("bad".to_string());
        let output = test_render(&record, &tokens, &color, &args);
        assert!(!output.contains('\x1b'));
    }

    #[test]
    fn render_sanitizes_stack_trace_with_escape_codes() {
        let mut record = make_record(Some(Level::Error), None, None, Some("fail"));
        record.stack_trace = Some("Error\x1b[31m at line 1\x1b[0m".to_string());
        let tokens = parse_template("{level}: {message}");
        let color = ColorConfig::with_enabled(false);
        let args = default_args();
        let output = test_render(&record, &tokens, &color, &args);
        assert!(!output.contains('\x1b'));
        assert!(output.contains("Error[31m at line 1[0m"));
    }

    // --- shorten_logger_dots tests ---

    #[test]
    fn shorten_logger_dots_empty() {
        assert_eq!(shorten_logger_dots(""), "");
    }

    #[test]
    fn shorten_logger_dots_single_segment() {
        assert_eq!(shorten_logger_dots("MyHandler"), "MyHandler");
    }

    #[test]
    fn shorten_logger_dots_two_segments() {
        assert_eq!(shorten_logger_dots("example.MyHandler"), "e.MyHandler");
    }

    #[test]
    fn shorten_logger_dots_many_segments() {
        assert_eq!(
            shorten_logger_dots("com.example.service.MyHandler"),
            "c.e.s.MyHandler"
        );
    }

    #[test]
    fn shorten_logger_dots_already_short() {
        assert_eq!(shorten_logger_dots("c.e.s.MyHandler"), "c.e.s.MyHandler");
    }

    #[test]
    fn shorten_logger_dots_three_segments() {
        assert_eq!(shorten_logger_dots("org.apache.Logger"), "o.a.Logger");
    }

    // --- truncate_logger_left tests ---

    #[test]
    fn truncate_logger_left_shorter_than_max() {
        assert_eq!(truncate_logger_left("c.e.s.MyHandler", 30), "c.e.s.MyHandler");
    }

    #[test]
    fn truncate_logger_left_exactly_at_max() {
        assert_eq!(truncate_logger_left("c.e.s.MyHandler", 15), "c.e.s.MyHandler");
    }

    #[test]
    fn truncate_logger_left_longer_with_dots() {
        // "c.e.s.MyHandler" is 15 chars, max 13 -> strip "c." -> "e.s.MyHandler" (13 chars)
        assert_eq!(truncate_logger_left("c.e.s.MyHandler", 13), "e.s.MyHandler");
    }

    #[test]
    fn truncate_logger_left_strip_multiple_segments() {
        // "com.example.service.Handler" is 27 chars, max 15 -> strip "com." -> "example.service.Handler" (23) -> strip "example." -> "service.Handler" (15)
        assert_eq!(
            truncate_logger_left("com.example.service.Handler", 15),
            "service.Handler"
        );
    }

    #[test]
    fn truncate_logger_left_longer_without_dots() {
        // No dots, must hard-truncate from left
        assert_eq!(truncate_logger_left("VeryLongLoggerName", 4), "Name");
    }

    #[test]
    fn truncate_logger_left_hard_truncate_after_dot_stripping() {
        // "a.VeryLongSegment" -> strip "a." -> "VeryLongSegment" (15 chars), max 4 -> hard truncate -> "ment"
        assert_eq!(truncate_logger_left("a.VeryLongSegment", 4), "ment");
    }

    #[test]
    fn truncate_logger_left_zero_means_unlimited() {
        assert_eq!(
            truncate_logger_left("com.example.service.MyHandler", 0),
            "com.example.service.MyHandler"
        );
    }

    #[test]
    fn truncate_logger_left_empty_string() {
        assert_eq!(truncate_logger_left("", 10), "");
    }

    // --- Render with LoggerFormat tests ---

    #[test]
    fn render_logger_format_as_is() {
        let record = make_record(
            Some(Level::Info),
            None,
            Some("com.example.service.MyHandler"),
            Some("hello"),
        );
        let tokens = parse_template("[{logger}] {message}");
        let color = ColorConfig::with_enabled(false);
        let mut args = default_args();
        args.logger_format = LoggerFormat::AsIs;
        let output = test_render(&record, &tokens, &color, &args);
        assert_eq!(output, "[com.example.service.MyHandler] hello");
    }

    #[test]
    fn render_logger_format_short_dots() {
        let record = make_record(
            Some(Level::Info),
            None,
            Some("com.example.service.MyHandler"),
            Some("hello"),
        );
        let tokens = parse_template("[{logger}] {message}");
        let color = ColorConfig::with_enabled(false);
        let mut args = default_args();
        args.logger_format = LoggerFormat::ShortDots;
        let output = test_render(&record, &tokens, &color, &args);
        assert_eq!(output, "[c.e.s.MyHandler] hello");
    }

    #[test]
    fn render_logger_format_short_dots_single_segment() {
        let record = make_record(Some(Level::Info), None, Some("SimpleLogger"), Some("msg"));
        let tokens = parse_template("[{logger}] {message}");
        let color = ColorConfig::with_enabled(false);
        let mut args = default_args();
        args.logger_format = LoggerFormat::ShortDots;
        let output = test_render(&record, &tokens, &color, &args);
        assert_eq!(output, "[SimpleLogger] msg");
    }

    #[test]
    fn render_logger_length_truncation() {
        let record = make_record(
            Some(Level::Info),
            None,
            Some("com.example.service.MyHandler"),
            Some("hello"),
        );
        let tokens = parse_template("[{logger}] {message}");
        let color = ColorConfig::with_enabled(false);
        let mut args = default_args();
        args.logger_format = LoggerFormat::AsIs;
        args.logger_length = 15;
        let output = test_render(&record, &tokens, &color, &args);
        // "com.example.service.MyHandler" -> strip "com." -> "example.service.MyHandler" -> strip "example." -> "service.MyHandler" (17, still too long) -> strip "service." -> "MyHandler" (9, fits)
        assert_eq!(output, "[MyHandler] hello");
    }

    #[test]
    fn render_logger_length_with_short_dots() {
        let record = make_record(
            Some(Level::Info),
            None,
            Some("com.example.service.MyHandler"),
            Some("hello"),
        );
        let tokens = parse_template("[{logger}] {message}");
        let color = ColorConfig::with_enabled(false);
        let mut args = default_args();
        args.logger_format = LoggerFormat::ShortDots;
        args.logger_length = 13;
        let output = test_render(&record, &tokens, &color, &args);
        // ShortDots: "c.e.s.MyHandler" (15 chars) -> truncate to 13 -> strip "c." -> "e.s.MyHandler" (13, fits)
        assert_eq!(output, "[e.s.MyHandler] hello");
    }

    #[test]
    fn render_logger_format_short_dots_missing_logger() {
        let record = make_record(Some(Level::Info), None, None, Some("msg"));
        let tokens = parse_template("[{logger}] {message}");
        let color = ColorConfig::with_enabled(false);
        let mut args = default_args();
        args.logger_format = LoggerFormat::ShortDots;
        let output = test_render(&record, &tokens, &color, &args);
        assert_eq!(output, "[] msg");
    }

    // --- Colored extra field key/value tests ---

    #[test]
    fn render_compact_extras_colored_key_value() {
        let mut record = make_record(Some(Level::Info), None, None, Some("test"));
        record.extras.insert("host".to_string(), json!("server1"));
        let tokens = parse_template("{level}: {message}");
        let color = ColorConfig::with_enabled(true);
        let mut args = default_args();
        args.add_fields = Some("host".to_string());
        let output = test_render(&record, &tokens, &color, &args);
        // Key should be magenta (\x1b[35m), value should be cyan (\x1b[36m)
        assert!(output.contains("\x1b[35m"));
        assert!(output.contains("\x1b[36m"));
        assert!(output.contains("host"));
        assert!(output.contains("server1"));
        // The = separator itself should NOT have ANSI codes (no dimmed \x1b[2m on it)
    }

    #[test]
    fn render_compact_extras_plain_no_color() {
        let mut record = make_record(Some(Level::Info), None, None, Some("test"));
        record.extras.insert("host".to_string(), json!("server1"));
        let tokens = parse_template("{level}: {message}");
        let color = ColorConfig::with_enabled(false);
        let mut args = default_args();
        args.add_fields = Some("host".to_string());
        let output = test_render(&record, &tokens, &color, &args);
        // No ANSI codes should appear when color is disabled
        assert!(!output.contains("\x1b["));
        assert!(output.contains("host=server1"));
    }

    #[test]
    fn render_expanded_extras_colored_key_value() {
        let mut record = make_record(Some(Level::Info), None, None, Some("test"));
        record.extras.insert("host".to_string(), json!("server1"));
        let tokens = parse_template("{level}: {message}");
        let color = ColorConfig::with_enabled(true);
        let mut args = default_args();
        args.expanded = true;
        args.add_fields = Some("host".to_string());
        let output = test_render(&record, &tokens, &color, &args);
        // Key should be magenta (\x1b[35m), value should be cyan (\x1b[36m)
        assert!(output.contains("\x1b[35m"));
        assert!(output.contains("\x1b[36m"));
        assert!(output.contains("host"));
        assert!(output.contains("server1"));
    }

    #[test]
    fn render_expanded_extras_plain_no_color() {
        let mut record = make_record(Some(Level::Info), None, None, Some("test"));
        record.extras.insert("host".to_string(), json!("server1"));
        let tokens = parse_template("{level}: {message}");
        let color = ColorConfig::with_enabled(false);
        let mut args = default_args();
        args.expanded = true;
        args.add_fields = Some("host".to_string());
        let output = test_render(&record, &tokens, &color, &args);
        // No ANSI codes should appear when color is disabled
        assert!(!output.contains("\x1b["));
        assert!(output.contains("\n  host: server1"));
    }
}
