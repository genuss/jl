use std::path::PathBuf;

use clap::{Parser, ValueEnum};

use crate::level::Level;

/// A JSON log pretty-printer.
///
/// Reads JSON log lines from stdin or files and renders them as
/// human-readable, colorized terminal output.
#[derive(Debug, Parser)]
#[command(name = "jl", version, about)]
pub struct Args {
    /// Output format template. Use {field} placeholders for substitution.
    #[arg(
        short,
        long,
        default_value = "{timestamp} {level} [{logger}] {message}"
    )]
    pub format: String,

    /// Comma-separated list of extra fields to include in output.
    #[arg(long, conflicts_with = "omit_fields")]
    pub add_fields: Option<String>,

    /// Comma-separated list of extra fields to omit from output
    /// (does not affect fields referenced in the format template).
    #[arg(long, conflicts_with = "add_fields")]
    pub omit_fields: Option<String>,

    /// Colorize output.
    #[arg(long, value_enum, default_value_t = ColorMode::Auto)]
    pub color: ColorMode,

    /// How to handle non-JSON input lines.
    #[arg(long, value_enum, default_value_t = NonJsonMode::PrintAsIs)]
    pub non_json: NonJsonMode,

    /// Force a specific log schema instead of auto-detecting.
    #[arg(long, value_enum, default_value_t = SchemaChoice::Auto)]
    pub schema: SchemaChoice,

    /// How to format logger names in output.
    #[arg(long, value_enum, default_value_t = LoggerFormat::ShortDots)]
    pub logger_format: LoggerFormat,

    /// Maximum display length for logger names (crops from left when exceeded).
    #[arg(long, default_value_t = 30)]
    pub logger_length: usize,

    /// How to format timestamps in output.
    #[arg(long, value_enum, default_value_t = TsFormat::Time)]
    pub ts_format: TsFormat,

    /// Minimum log level to display. Lines below this level are filtered out.
    #[arg(long)]
    pub min_level: Option<Level>,

    /// Output records as raw JSON instead of formatted text.
    #[arg(long)]
    pub raw_json: bool,

    /// Show extra fields on separate lines instead of the default compact (same-line) mode.
    #[arg(long)]
    pub expanded: bool,

    /// Color for extra field keys.
    #[arg(long, value_enum, default_value_t = CliColor::Magenta)]
    pub key_color: CliColor,

    /// Color for extra field values.
    #[arg(long, value_enum, default_value_t = CliColor::Cyan)]
    pub value_color: CliColor,

    /// Timezone for displaying timestamps (local, utc, or IANA name).
    #[arg(long, default_value = "local")]
    pub tz: String,

    /// Follow the last input file, waiting for new data (like tail -f).
    /// When multiple files are given, preceding files are read to completion first.
    #[arg(long)]
    pub follow: bool,

    /// Write output to a file instead of stdout.
    #[arg(short = 'o', long)]
    pub output: Option<PathBuf>,

    /// Generate shell completion script and exit.
    #[arg(long, value_enum)]
    pub completions: Option<Shell>,

    /// Input file(s) to read. If omitted, reads from stdin.
    #[arg()]
    pub files: Vec<PathBuf>,
}

/// Controls when ANSI color codes are emitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ColorMode {
    /// Colorize if stdout is a terminal.
    Auto,
    /// Always emit color codes.
    Always,
    /// Never emit color codes.
    Never,
}

/// How to handle lines that are not valid JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum NonJsonMode {
    /// Print non-JSON lines to output (control characters are sanitized).
    PrintAsIs,
    /// Silently skip non-JSON lines.
    Skip,
    /// Exit with an error on the first non-JSON line.
    Fail,
}

/// How to format logger names in output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum LoggerFormat {
    /// Abbreviate dot-separated segments: `com.example.service.Handler` → `c.e.s.Handler`.
    ShortDots,
    /// Display the logger name exactly as it appears in the log record.
    AsIs,
}

/// How to format timestamps in output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum TsFormat {
    /// Show time only: `HH:MM:SS.mmm`.
    Time,
    /// Show full date and time: `YYYY-MM-DDTHH:MM:SS.mmm`.
    Full,
}

/// Which log schema to use for field mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SchemaChoice {
    /// Automatically detect the schema from the first JSON line.
    Auto,
    /// Logstash JSON format.
    Logstash,
    /// Logrus JSON format.
    Logrus,
    /// Bunyan JSON format.
    Bunyan,
    /// Generic fallback with common field name guessing.
    Generic,
}

/// Shell for which to generate completion scripts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Shell {
    /// Bash shell completions.
    Bash,
    /// Zsh shell completions.
    Zsh,
    /// Fish shell completions.
    Fish,
}

/// ANSI color choice for styled output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CliColor {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_args(args: &[&str]) -> Args {
        Args::parse_from(args)
    }

    #[test]
    fn defaults_no_args() {
        let args = parse_args(&["jl"]);
        assert_eq!(args.format, "{timestamp} {level} [{logger}] {message}");
        assert!(args.add_fields.is_none());
        assert!(args.omit_fields.is_none());
        assert_eq!(args.color, ColorMode::Auto);
        assert_eq!(args.non_json, NonJsonMode::PrintAsIs);
        assert_eq!(args.schema, SchemaChoice::Auto);
        assert_eq!(args.logger_format, LoggerFormat::ShortDots);
        assert_eq!(args.logger_length, 30);
        assert_eq!(args.ts_format, TsFormat::Time);
        assert!(args.min_level.is_none());
        assert!(!args.raw_json);
        assert!(!args.expanded);
        assert_eq!(args.key_color, CliColor::Magenta);
        assert_eq!(args.value_color, CliColor::Cyan);
        assert_eq!(args.tz, "local");
        assert!(!args.follow);
        assert!(args.output.is_none());
        assert!(args.completions.is_none());
        assert!(args.files.is_empty());
    }

    #[test]
    fn custom_format() {
        let args = parse_args(&["jl", "--format", "{level}: {message}"]);
        assert_eq!(args.format, "{level}: {message}");
    }

    #[test]
    fn short_format() {
        let args = parse_args(&["jl", "-f", "{message}"]);
        assert_eq!(args.format, "{message}");
    }

    #[test]
    fn add_fields_only() {
        let args = parse_args(&["jl", "--add-fields", "host,pid"]);
        assert_eq!(args.add_fields.as_deref(), Some("host,pid"));
        assert!(args.omit_fields.is_none());
    }

    #[test]
    fn omit_fields_only() {
        let args = parse_args(&["jl", "--omit-fields", "stack_trace"]);
        assert!(args.add_fields.is_none());
        assert_eq!(args.omit_fields.as_deref(), Some("stack_trace"));
    }

    #[test]
    fn add_and_omit_fields_conflict() {
        let result = Args::try_parse_from([
            "jl",
            "--add-fields",
            "host",
            "--omit-fields",
            "stack_trace",
        ]);
        assert!(result.is_err());
    }

    #[test]
    fn color_modes() {
        let args = parse_args(&["jl", "--color", "always"]);
        assert_eq!(args.color, ColorMode::Always);

        let args = parse_args(&["jl", "--color", "never"]);
        assert_eq!(args.color, ColorMode::Never);

        let args = parse_args(&["jl", "--color", "auto"]);
        assert_eq!(args.color, ColorMode::Auto);
    }

    #[test]
    fn non_json_modes() {
        let args = parse_args(&["jl", "--non-json", "print-as-is"]);
        assert_eq!(args.non_json, NonJsonMode::PrintAsIs);

        let args = parse_args(&["jl", "--non-json", "skip"]);
        assert_eq!(args.non_json, NonJsonMode::Skip);

        let args = parse_args(&["jl", "--non-json", "fail"]);
        assert_eq!(args.non_json, NonJsonMode::Fail);
    }

    #[test]
    fn schema_choices() {
        let args = parse_args(&["jl", "--schema", "auto"]);
        assert_eq!(args.schema, SchemaChoice::Auto);

        let args = parse_args(&["jl", "--schema", "logstash"]);
        assert_eq!(args.schema, SchemaChoice::Logstash);

        let args = parse_args(&["jl", "--schema", "logrus"]);
        assert_eq!(args.schema, SchemaChoice::Logrus);

        let args = parse_args(&["jl", "--schema", "bunyan"]);
        assert_eq!(args.schema, SchemaChoice::Bunyan);

        let args = parse_args(&["jl", "--schema", "generic"]);
        assert_eq!(args.schema, SchemaChoice::Generic);
    }

    #[test]
    fn min_level_parsing() {
        let args = parse_args(&["jl", "--min-level", "warn"]);
        assert_eq!(args.min_level, Some(Level::Warn));

        let args = parse_args(&["jl", "--min-level", "INFO"]);
        assert_eq!(args.min_level, Some(Level::Info));

        let args = parse_args(&["jl", "--min-level", "Error"]);
        assert_eq!(args.min_level, Some(Level::Error));
    }

    #[test]
    fn boolean_flags() {
        let args = parse_args(&["jl", "--raw-json", "--expanded", "--follow"]);
        assert!(args.raw_json);
        assert!(args.expanded);
        assert!(args.follow);
    }

    #[test]
    fn timezone() {
        let args = parse_args(&["jl", "--tz", "utc"]);
        assert_eq!(args.tz, "utc");

        let args = parse_args(&["jl", "--tz", "America/New_York"]);
        assert_eq!(args.tz, "America/New_York");
    }

    #[test]
    fn output_file() {
        let args = parse_args(&["jl", "-o", "/tmp/out.log"]);
        assert_eq!(args.output, Some(PathBuf::from("/tmp/out.log")));

        let args = parse_args(&["jl", "--output", "/tmp/out.log"]);
        assert_eq!(args.output, Some(PathBuf::from("/tmp/out.log")));
    }

    #[test]
    fn input_files() {
        let args = parse_args(&["jl", "app.log", "error.log"]);
        assert_eq!(
            args.files,
            vec![PathBuf::from("app.log"), PathBuf::from("error.log")]
        );
    }

    #[test]
    fn combined_options() {
        let args = parse_args(&[
            "jl",
            "--format",
            "{level} {message}",
            "--color",
            "never",
            "--min-level",
            "debug",
            "--schema",
            "bunyan",
            "--tz",
            "utc",
            "--expanded",
            "-o",
            "/tmp/out.txt",
            "input.log",
        ]);
        assert_eq!(args.format, "{level} {message}");
        assert_eq!(args.color, ColorMode::Never);
        assert_eq!(args.min_level, Some(Level::Debug));
        assert_eq!(args.schema, SchemaChoice::Bunyan);
        assert_eq!(args.tz, "utc");
        assert!(args.expanded);
        assert_eq!(args.output, Some(PathBuf::from("/tmp/out.txt")));
        assert_eq!(args.files, vec![PathBuf::from("input.log")]);
    }

    #[test]
    fn completions_bash() {
        let args = parse_args(&["jl", "--completions", "bash"]);
        assert_eq!(args.completions, Some(Shell::Bash));
    }

    #[test]
    fn completions_zsh() {
        let args = parse_args(&["jl", "--completions", "zsh"]);
        assert_eq!(args.completions, Some(Shell::Zsh));
    }

    #[test]
    fn completions_fish() {
        let args = parse_args(&["jl", "--completions", "fish"]);
        assert_eq!(args.completions, Some(Shell::Fish));
    }

    #[test]
    fn completions_invalid_value_fails() {
        let result = Args::try_parse_from(["jl", "--completions", "powershell"]);
        assert!(result.is_err());
    }
}
